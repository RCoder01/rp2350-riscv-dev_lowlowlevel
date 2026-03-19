#![allow(non_snake_case)]
use core::{iter::zip, marker::PhantomData, num::NonZeroU8, ptr};

use crate::{
    assert, assert_eq, blink_partial_value,
    clocks::{PLL_SYS_PARAMS, PLL_USB_PARAMS},
    common::{AliasedRegister, copy_const, csr_clear, csr_set, nop_volatile},
    const_for, delay,
    resets::{RESETS_RESET, RESETS_RESET_DONE},
    trap::{RVCSR_MEIEA, RVCSR_MEIFA},
    usb::cdc::{AbstractControlManagement, CallManagement, CommunicationsDescriptor, Union},
};

const RESETS_RESET_USBCTRL: u32 = 28;
const USBCTRL_MASK: u32 = 1 << RESETS_RESET_USBCTRL;

const USBCTRL_DPRAM_BASE: *mut u8 = ptr::without_provenance_mut(0x5010_0000);
const USBCTRL_DPRAM_LEN: usize = 4 * 1024;
const USBCTRL_DPRAM: *mut [u8; USBCTRL_DPRAM_LEN] = USBCTRL_DPRAM_BASE.cast();

#[derive(Copy, Clone)]
pub struct DPRAMPtr<T> {
    byte_offset: u16,
    _type: PhantomData<*mut T>,
}

impl<T> DPRAMPtr<T> {
    pub const fn new(byte_offset: u16) -> Self {
        assert!((byte_offset as usize) < USBCTRL_DPRAM_LEN);
        assert!((byte_offset as usize) + size_of::<T>() <= USBCTRL_DPRAM_LEN);
        Self {
            byte_offset,
            _type: PhantomData,
        }
    }

    pub const fn cast<U>(self) -> DPRAMPtr<U> {
        DPRAMPtr::new(self.byte_offset)
    }

    pub fn byte_offset(self) -> u16 {
        self.byte_offset
    }

    pub fn write(self, val: T)
    where
        T: Copy, // Don't want to have to deal with T drop issues
    {
        let ptr = USBCTRL_DPRAM_BASE.wrapping_byte_offset(self.byte_offset as isize);
        // are unaligned writes fine?
        unsafe { ptr.cast::<T>().write_volatile(val) };
    }

    pub fn read(self) -> T
    where
        T: Copy, // Don't want to have to deal with T drop issues
    {
        let ptr = USBCTRL_DPRAM_BASE.wrapping_byte_offset(self.byte_offset as isize);
        // are unaligned reads fine?
        unsafe { ptr.cast::<T>().read_volatile() }
    }

    pub const fn offset_bytes(self, offset: i16) -> Self {
        Self::new(self.byte_offset.wrapping_add_signed(offset))
    }

    pub const fn offset(self, offset: i16) -> Self {
        let Some(byte_offset) = (size_of::<T>() as isize).checked_mul(offset as isize) else {
            panic!("overflow in offset computation")
        };
        if !((i16::MIN as isize) <= byte_offset && byte_offset <= (i16::MAX as isize)) {
            panic!("offset is too large");
        }
        self.offset_bytes(byte_offset as i16)
    }
}

const USBCTRL_REG_BASE: AliasedRegister = unsafe { AliasedRegister::from_addr(0x5011_0000) };
const USBCTRL_ADDR_ENDP: AliasedRegister = unsafe { USBCTRL_REG_BASE.offset_bytes(0x000) };
const USBCTRL_MAIN_CTRL: AliasedRegister = unsafe { USBCTRL_REG_BASE.offset_bytes(0x040) };
const USBCTRL_SIE_CTRL: AliasedRegister = unsafe { USBCTRL_REG_BASE.offset_bytes(0x04C) };
const USBCTRL_SIE_STATUS: AliasedRegister = unsafe { USBCTRL_REG_BASE.offset_bytes(0x050) };
const USBCTRL_BUF_STATUS: AliasedRegister = unsafe { USBCTRL_REG_BASE.offset_bytes(0x058) };
const USBCTRL_USB_MUXING: AliasedRegister = unsafe { USBCTRL_REG_BASE.offset_bytes(0x074) };
const USBCTRL_USB_PWR: AliasedRegister = unsafe { USBCTRL_REG_BASE.offset_bytes(0x078) };
const USBCTRL_INTE: AliasedRegister = unsafe { USBCTRL_REG_BASE.offset_bytes(0x090) };
const USBCTRL_INTS: AliasedRegister = unsafe { USBCTRL_REG_BASE.offset_bytes(0x098) };

fn reset_usb() {
    RESETS_RESET.set(USBCTRL_MASK);
    RESETS_RESET.clear(USBCTRL_MASK);
    while RESETS_RESET_DONE.read() & USBCTRL_MASK == 0 {
        nop_volatile();
    }
}

const USB_DPSRAM_WAIT_CYCLES: u32 = PLL_SYS_PARAMS
    .f_out_post_div_hz()
    .div_ceil(PLL_USB_PARAMS.f_out_post_div_hz());

fn wait_sync_usb_dpsram() {
    for _ in 0..USB_DPSRAM_WAIT_CYCLES {
        nop_volatile()
    }
}

const USBCTRL_IRQ: u32 = 14;

const USBINT_BUF_STATUS: u32 = 1 << 4;
const USBINT_BUS_RESET: u32 = 1 << 12;
const USBINT_SETUP_REQ: u32 = 1 << 16;

fn enable_usbctrl_interrupt() {
    // lower 4 bits specify which 16-bit window, upper 16 bits mask the window
    let usbctrl = (((1 << (USBCTRL_IRQ % 16)) << 16) + (USBCTRL_IRQ / 16)) as usize;
    unsafe { csr_clear::<RVCSR_MEIFA>(usbctrl) }; // remove any pending forced interrupts
    unsafe { csr_set::<RVCSR_MEIEA>(usbctrl) }; // enable usbctrl interrupt
}

pub fn init_usb_as_device() {
    reset_usb();
    unsafe {
        USBCTRL_DPRAM.write_volatile([0; _]);
    }
    enable_usbctrl_interrupt();

    const MUXING_TO_PHY: u32 = 1 << 0;
    const MUXING_SOFTCON: u32 = 1 << 3;
    // Mux the controller to the onboard usb phy
    USBCTRL_USB_MUXING.write(MUXING_TO_PHY | MUXING_SOFTCON | 0);

    const PWR_VBUS_DETECT: u32 = 1 << 2;
    const PWR_VBUS_DETECT_OVERRIDE_ENABLE: u32 = 1 << 3;
    // Force VBUS detect so the device thinks it is plugged into a host
    USBCTRL_USB_PWR.write(PWR_VBUS_DETECT | PWR_VBUS_DETECT_OVERRIDE_ENABLE | 0);

    const MAIN_CTRL_CONTROLLER_ENABLE: u32 = 1 << 0;
    const MAIN_CTRL_HOST_OR_DEVICE: u32 = 1 << 1;
    const MAIN_CTRL_PHY_ISOLATE: u32 = 1 << 2;
    // Enable the USB controller in device mode.
    USBCTRL_MAIN_CTRL.write(
        (MAIN_CTRL_PHY_ISOLATE * 0)
            | (MAIN_CTRL_HOST_OR_DEVICE * 0)
            | MAIN_CTRL_CONTROLLER_ENABLE
            | 0,
    );

    const SIE_EP0_INT_BUF: u32 = 1 << 29;
    const SIE_PULLDOWN_ENABLE: u32 = 1 << 15;
    // Enable an interrupt per EP0 transaction, disable pulldown
    USBCTRL_SIE_CTRL.write(SIE_EP0_INT_BUF | (SIE_PULLDOWN_ENABLE * 0) | 0);

    // Enable interrupts for when a buffer is done, when the bus is reset,
    // and when a setup packet is received
    USBCTRL_INTE.write(USBINT_BUF_STATUS | USBINT_BUS_RESET | USBINT_SETUP_REQ | 0);

    // DEVICE_CONFIG.configure_endpoints();

    const SIE_PULLUP_ENABLE: u32 = 1 << 16;
    // Present full speed device by enabling pull up on DP
    USBCTRL_SIE_CTRL.set(SIE_PULLUP_ENABLE);
}

const EP0_OUT_BUFFER_CTRL_REG: DPRAMPtr<u32> =
    endpoint_buffer_ctrl_register(EndpointId::Endpoint0, Direction::Out);
const EP0_IN_BUFFER_CTRL_REG: DPRAMPtr<u32> =
    endpoint_buffer_ctrl_register(EndpointId::Endpoint0, Direction::In);

const EP0_BUFFER: DPRAMPtr<[u8; 0x40]> = DPRAMPtr::new(0x100);
const BUFFER_0_AVAILABLE: u32 = 1 << 10;
const BUFFER_0_DATA_PID: u32 = 1 << 13;
const BUFFER_0_FULL: u32 = 1 << 15;

fn acknowledge_out_request() {
    let pid = (!EP0_IN_BUFFER_CTRL_REG.read()) & BUFFER_0_DATA_PID;
    EP0_IN_BUFFER_CTRL_REG.write(pid | BUFFER_0_AVAILABLE | BUFFER_0_FULL);
}

fn send_device_descriptor(packet: &SetupPacket) {
    let descriptor = DEVICE_CONFIG.device_descriptor;
    let mut buf = [0; _];
    descriptor.write(&mut buf);
    EP0_BUFFER.cast().write(buf);
    wait_sync_usb_dpsram();
    EP0_IN_BUFFER_CTRL_REG.write(
        (DeviceDescriptor::LENGTH as u32).min(packet.length as u32)
            | BUFFER_0_AVAILABLE
            | BUFFER_0_DATA_PID
            | BUFFER_0_FULL,
    );
}

pub const FLAG: DPRAMPtr<u8> = DPRAMPtr::new((USBCTRL_DPRAM_LEN - 2) as u16);
const EP0_BUF_STATUS: DPRAMPtr<u8> = DPRAMPtr::new((USBCTRL_DPRAM_LEN - 1) as u16);

const CONFIG_DESC_SIZE: usize = DEVICE_CONFIG.configuration_descriptor.wTotalLength as usize;
fn config_desc_packet(total_len: usize, part: usize) -> &'static [u8] {
    const fn split<'a, const N: usize>(slice: &mut &'a mut [u8]) -> &'a mut [u8; N] {
        let copy = core::mem::replace(slice, &mut []);
        let (front, back) = copy.split_at_mut(N);
        *slice = back;
        front.as_mut_array().expect("sizes should match")
    }

    const CONFIG_DESC_BUF: [u8; CONFIG_DESC_SIZE] = {
        let mut buf = [0; CONFIG_DESC_SIZE];
        let rem = &mut buf.as_mut_slice();
        DEVICE_CONFIG.configuration_descriptor.write(split(rem));
        INTERFACE_ASSOCIATION_DESCRIPTORS[0].write(split(rem));
        INTERFACE_DESCRIPTORS[0].write(split(rem));
        const_for!(comms_desc in COMMUNICATIONS_DESCRIPTORS => {
            let Ok(_) = comms_desc.try_write(rem) else {
                panic!("should be large enough");
            };
        });
        const_for!(endpoint in DEVICE_CONFIG.endpoints => {
            if matches!(endpoint.endpoint(), EndpointId::Endpoint1) {
                endpoint.write(split(rem));
            }
        });
        INTERFACE_DESCRIPTORS[1].write(split(rem));
        const_for!(endpoint in DEVICE_CONFIG.endpoints => {
            if matches!(endpoint.endpoint(), EndpointId::Endpoint2) {
                endpoint.write(split(rem));
            }
        });
        assert_eq!(rem.len(), 0);
        buf
    };
    let start = part * 64;
    let end = (start + 64).min(total_len).min(CONFIG_DESC_BUF.len());
    if end < start {
        &[]
    } else {
        &CONFIG_DESC_BUF.as_slice()[start..end]
    }
}

fn send_config_descriptor(packet: &SetupPacket, part_index: usize) {
    let part = config_desc_packet(packet.length as usize, part_index);
    if part.len() == 64 {
        EP0_BUFFER.write(part.try_into().unwrap());
        EP0_BUF_STATUS.write((part_index + 1) as u8);
    } else {
        for (i, b) in part.iter().copied().enumerate() {
            EP0_BUFFER.cast().offset(i as _).write(b)
        }
        EP0_BUF_STATUS.write(0);
    }
    wait_sync_usb_dpsram();
    const CTRL: DPRAMPtr<u32> = endpoint_buffer_ctrl_register(EndpointId::Endpoint0, Direction::In);
    let pid = (!CTRL.read()) & BUFFER_0_DATA_PID;
    let ctrl_val = part.len() as u32 | BUFFER_0_AVAILABLE | BUFFER_0_FULL | pid;
    CTRL.write(ctrl_val);
}

fn send_string_descriptor(packet: &SetupPacket) {
    let i = (packet.value & 0xFF) as u8;
    const PTR: DPRAMPtr<[u8; 64]> = endpoint_buffer(EndpointId::Endpoint0, Direction::In).cast();
    let len = if i == 0 {
        let buf = DEVICE_CONFIG.lang_descriptor;
        PTR.cast().write(buf);
        buf.len() as u8
    } else {
        let str = DEVICE_CONFIG
            .descriptor_strings
            .get(i as usize - 1)
            .copied()
            .unwrap_or("Unknown string");
        let str_len = str.encode_utf16().count() * size_of::<u16>();
        let len = (str_len + 2).min(64) as u8;
        let mut buf = [0u16; 32];
        buf[0] = u16::from_le_bytes([len, DescriptorType::String as u8]);
        for (char, slot) in zip(str.encode_utf16(), &mut buf[1..]) {
            *slot = char;
        }
        let buf_u8 = buf.map(|v| v.to_le_bytes());
        PTR.write(buf_u8.as_flattened().try_into().unwrap());
        len
    };
    wait_sync_usb_dpsram();

    const CTRL: DPRAMPtr<u32> = endpoint_buffer_ctrl_register(EndpointId::Endpoint0, Direction::In);
    let pid = (!CTRL.read()) & BUFFER_0_DATA_PID;
    let ctrl_val = len as u32 | BUFFER_0_AVAILABLE | BUFFER_0_FULL | pid;
    CTRL.write(ctrl_val);
}

fn send_device_qualifier_descriptor(_packet: &SetupPacket) {
    let len = 0;
    const CTRL: DPRAMPtr<u32> = endpoint_buffer_ctrl_register(EndpointId::Endpoint0, Direction::In);
    let pid = (!CTRL.read()) & BUFFER_0_DATA_PID;
    let ctrl_val = len as u32 | BUFFER_0_AVAILABLE | BUFFER_0_FULL | pid;
    CTRL.write(ctrl_val);
}

const SIE_STATUS_SETUP_REC: u32 = 1 << 17;
const SIE_STATUS_BUS_RESET: u32 = 1 << 19;
pub fn usb_trap_handler() {
    const NEW_ADDR: *mut u8 = 0x20000000 as *mut u8;
    let mut status = USBCTRL_INTS.read();
    if FLAG.read() != 0 {
        FLAG.write(FLAG.read() + 1);
    }

    if (status & USBINT_SETUP_REQ) != 0 {
        status ^= USBINT_SETUP_REQ;
        USBCTRL_SIE_STATUS.clear(SIE_STATUS_SETUP_REC);
        // set curr pid to 0 so next pid will be 1
        EP0_IN_BUFFER_CTRL_REG.write(EP0_IN_BUFFER_CTRL_REG.read() & !BUFFER_0_DATA_PID);
        let packet = DPRAM_SETUP_PACKET.read();
        match packet.request_direction() {
            Direction::Out => match packet.request() {
                Ok(Request::SetAddress) => {
                    let new_addr = (packet.value & 0x7F) as u8 + 0x80;
                    unsafe { NEW_ADDR.write(new_addr) };
                    acknowledge_out_request();
                }
                Ok(Request::SetConfiguration) => {
                    assert!(packet.value == 1); // device only has one configuration
                    acknowledge_out_request();

                    const CTRL_REG: DPRAMPtr<u32> =
                        endpoint_buffer_ctrl_register(EndpointId::Endpoint1, Direction::Out);

                    let len = 64;
                    let pid = (!CTRL_REG.read()) & BUFFER_0_DATA_PID;
                    CTRL_REG.write(pid | BUFFER_0_AVAILABLE | len);
                }
                Err(0x20) => {
                    // set line coding request
                    acknowledge_out_request();
                    FLAG.write(1);
                }
                Err(0x22) => {
                    // set control line state
                    acknowledge_out_request();
                }
                _ => {
                    acknowledge_out_request();
                }
            },
            Direction::In => match packet.request() {
                Ok(Request::GetDescriptor) => match ((packet.value >> 8) as u8).try_into() {
                    Ok(DescriptorType::Device) => {
                        send_device_descriptor(&packet);
                    }
                    Ok(DescriptorType::Config) => send_config_descriptor(&packet, 0),
                    Ok(DescriptorType::String) => send_string_descriptor(&packet),
                    Ok(DescriptorType::DeviceQualifier) => {
                        send_device_qualifier_descriptor(&packet)
                    }
                    _ => todo!(),
                },
                _ => todo!(),
            },
        }
    }

    if (status & USBINT_BUF_STATUS) != 0 {
        status ^= USBINT_BUF_STATUS;
        let buffers = USBCTRL_BUF_STATUS.read();
        let mut remaining = buffers;

        while remaining != 0 {
            let bit = remaining.trailing_zeros();
            USBCTRL_BUF_STATUS.clear(1 << bit);
            remaining &= !(1 << bit);

            let endpoint = EndpointId::new(bit as u8 / 2).expect("bit/2 < 16");
            let dir = if (bit & 0b1) == 0 {
                Direction::In
            } else {
                Direction::Out
            };
            match (endpoint, dir) {
                (EndpointId::Endpoint0, Direction::Out) => {}
                (EndpointId::Endpoint0, Direction::In) => {
                    let new_addr = unsafe { NEW_ADDR.read() };
                    if new_addr & 0x80 != 0 {
                        unsafe { NEW_ADDR.write(new_addr & 0x7F) };
                        USBCTRL_ADDR_ENDP.write(new_addr as u32);
                    } else {
                        let status = EP0_BUF_STATUS.read();
                        if status != 0 {
                            let setup = DPRAM_SETUP_PACKET.read();
                            send_config_descriptor(&setup, status as usize);
                        } else {
                            let pid = (!EP0_OUT_BUFFER_CTRL_REG.read()) & BUFFER_0_DATA_PID;
                            EP0_OUT_BUFFER_CTRL_REG.write(pid | BUFFER_0_AVAILABLE);
                        }
                    }
                }
                (EndpointId::Endpoint1, Direction::In) => {
                    todo!()
                }
                (EndpointId::Endpoint2, Direction::In) => {
                    todo!()
                }
                (EndpointId::Endpoint2, Direction::Out) => {
                    todo!()
                    // let val = endpoint_buffer(EndpointId::Endpoint2, Direction::Out).read()[0];
                    // endpoint_buffer(EndpointId::Endpoint2, Direction::In)
                    //     .cast()
                    //     .write(val);
                    // const CTRL_REG: DPRAMPtr<u32> =
                    //     endpoint_buffer_ctrl_register(EndpointId::Endpoint2, Direction::In);
                    // let pid = (!CTRL_REG.read()) & BUFFER_0_DATA_PID;
                    // CTRL_REG.write(pid | BUFFER_0_AVAILABLE | BUFFER_0_FULL);
                }
                _ => unimplemented!(),
            }
        }
    }

    if (status & USBINT_BUS_RESET) != 0 {
        status ^= USBINT_BUS_RESET;
        USBCTRL_SIE_STATUS.clear(SIE_STATUS_BUS_RESET);
        unsafe { NEW_ADDR.write(0) };
        USBCTRL_ADDR_ENDP.write(0);
    }

    if status != 0 {
        blink_partial_value(3, 2);
        delay(10);
        panic!("Unhandled usb interrupt");
    }
}

const DPRAM_SETUP_PACKET: DPRAMPtr<SetupPacket> = DPRAMPtr::new(0);

#[derive(Copy, Clone)]
pub enum Direction {
    Out = 0x00,
    In = 0x80,
}

impl TryFrom<u8> for Direction {
    type Error = u8;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00 => Ok(Direction::Out),
            0x80 => Ok(Direction::In),
            _ => Err(value),
        }
    }
}

#[repr(u8)]
#[derive(Copy, Clone)]
enum Request {
    GetStatus = 0x0,
    ClearFeature = 0x01,
    SetFeature = 0x03,
    SetAddress = 0x05,
    GetDescriptor = 0x06,
    SetDescriptor = 0x07,
    GetConfiguration = 0x08,
    SetConfiguration = 0x09,
    GetInterface = 0x0A,
    SetInterface = 0x0B,
    SyncFrame = 0x0C,
}

impl TryFrom<u8> for Request {
    type Error = u8;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x0 => Ok(Request::GetStatus),
            0x01 => Ok(Request::ClearFeature),
            0x03 => Ok(Request::SetFeature),
            0x05 => Ok(Request::SetAddress),
            0x06 => Ok(Request::GetDescriptor),
            0x07 => Ok(Request::SetDescriptor),
            0x08 => Ok(Request::GetConfiguration),
            0x09 => Ok(Request::SetConfiguration),
            0x0A => Ok(Request::GetInterface),
            0x0B => Ok(Request::SetInterface),
            0x0C => Ok(Request::SyncFrame),
            _ => Err(value),
        }
    }
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
struct SetupPacket {
    request_type: u8,
    request: u8,
    value: u16,
    index: u16,
    length: u16,
}

impl SetupPacket {
    fn request_direction(self) -> Direction {
        Direction::try_from(self.request_type & 0x80).expect("Direction is either 0x00 or 0x80")
    }

    fn request(self) -> Result<Request, u8> {
        Request::try_from(self.request)
    }
}

const _: () = if size_of::<SetupPacket>() != 8 {
    panic!("Incorrect USBSetupPacket size")
};

const INTERFACE_DESCRIPTORS: [InterfaceDescriptor; 2] = [
    InterfaceDescriptor {
        bInterfaceNumber: 0,
        bAlternateSetting: 0,
        bNumEndpoints: 1,
        bInterfaceClass: 0x02,    // Communications and CDC Control
        bInterfaceSubClass: 0x02, // Abstract Control Model
        bInterfaceProtocol: 0,
        iInterface: 4,
    },
    InterfaceDescriptor {
        bInterfaceNumber: 1,
        bAlternateSetting: 0,
        bNumEndpoints: 2,
        bInterfaceClass: 0x0A, // CDC-Data
        bInterfaceSubClass: 0x00,
        bInterfaceProtocol: 0x00,
        iInterface: 0,
    },
];

const INTERFACE_ASSOCIATION_DESCRIPTORS: [InterfaceAssociationDescriptor; 1] =
    [InterfaceAssociationDescriptor {
        bFirstInterface: 0,
        bInterfaceCount: 2,
        bFunctionClass: 0x02, // Communications and CDC Control
        bFunctionSubClass: 0x02,
        bFunctionProtocol: 0,
        iFunction: 0,
    }];

const COMMUNICATIONS_DESCRIPTORS: [CommunicationsDescriptor; 4] = [
    CommunicationsDescriptor::Header,
    CommunicationsDescriptor::CallManagement(CallManagement {
        bmCapabilities: 0x00,
        data_interface: 0x01,
    }),
    CommunicationsDescriptor::AbstractControlManagement(AbstractControlManagement {
        bmCapabilities: 0x06,
    }),
    CommunicationsDescriptor::Union(Union {
        control_interface: 0,
        subordinate_interface: 1,
    }),
];

const ENDPOINTS: [EndpointDescriptor; 5] = [
    EndpointDescriptor {
        endpoint_address: Direction::Out as u8 | 0,
        attributes: TransferType::Control,
        max_packet_size: 64,
        interval: 0,
    },
    EndpointDescriptor {
        endpoint_address: Direction::In as u8 | 0,
        attributes: TransferType::Control,
        max_packet_size: 64,
        interval: 0,
    },
    EndpointDescriptor {
        endpoint_address: Direction::In as u8 | 1,
        attributes: TransferType::Interrupt,
        max_packet_size: 8,
        interval: 1,
    },
    EndpointDescriptor {
        endpoint_address: Direction::In as u8 | 2,
        attributes: TransferType::Bulk,
        max_packet_size: 64,
        interval: 0,
    },
    EndpointDescriptor {
        endpoint_address: Direction::Out as u8 | 2,
        attributes: TransferType::Bulk,
        max_packet_size: 64,
        interval: 0,
    },
];

const DEVICE_CONFIG: DeviceConfig<'static> = DeviceConfig {
    device_descriptor: &DeviceDescriptor {
        bcdUSB: 0x0200,      // USB 2.0 device
        bDeviceClass: 0xEF,  // Misc
        bDeviceSubClass: 2,  // No subclass
        bDeviceProtocol: 1,  // Interface association descriptor
        bMaxPacketSize0: 64, // Max packet size for ep0
        idVendor: 0x0000,    // Your vendor id
        idProduct: 0x0001,   // Your product ID
        bcdDevice: 0,        // No device revision number
        manufacturer: Some(StringDescriptorIndex(NonZeroU8::new(1).unwrap())), // Manufacturer string index
        product: Some(StringDescriptorIndex(NonZeroU8::new(2).unwrap())), // Product string index
        serial_number: None,                                              // No serial number
        bNumConfigurations: 1,                                            // One configuration
    },
    configuration_descriptor: &ConfigurationDescriptor {
        wTotalLength: (ConfigurationDescriptor::LENGTH as u16
            + (INTERFACE_DESCRIPTORS.len() * InterfaceDescriptor::LENGTH_USIZE) as u16
            + (INTERFACE_ASSOCIATION_DESCRIPTORS.len()
                * InterfaceAssociationDescriptor::LENGTH_USIZE) as u16
            + ((ENDPOINTS.len() - 2) * EndpointDescriptor::LENGTH_USIZE) as u16
            + {
                let mut sum = 0;
                const_for!(desc in COMMUNICATIONS_DESCRIPTORS => {
                    sum += desc.length() as u16;
                });
                sum
            }),
        bNumInterfaces: INTERFACE_DESCRIPTORS.len() as u8,
        bConfigurationValue: 1, // Configuration 1
        iConfiguration: 0,      // No string
        bmAttributes: 0xc0,     // attributes: self powered, no remote wakeup
        bMaxPower: 0x32,        // 100ma
    },
    lang_descriptor: [
        4,    // bLength
        0x03, // bDescriptorType == String Descriptor
        0x09, 0x04, // language id = us english
    ],
    descriptor_strings: &[
        "RCoder01's",             // Vendor
        "worlds worst USB stick", // Product
    ],
    endpoints: &ENDPOINTS,
};

#[repr(transparent)]
#[derive(Copy, Clone)]
struct StringDescriptorIndex(NonZeroU8);

#[repr(u8)]
#[derive(Copy, Clone)]
enum DescriptorType {
    Device = 0x01,
    Config = 0x02,
    String = 0x03,
    Interface = 0x04,
    Endpoint = 0x05,
    DeviceQualifier = 0x06,
    InterfaceAssociation = 0x0B,
    CSInterface = 0x24,
}

impl TryFrom<u8> for DescriptorType {
    type Error = u8;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x01 => Ok(Self::Device),
            0x02 => Ok(Self::Config),
            0x03 => Ok(Self::String),
            0x04 => Ok(Self::Interface),
            0x05 => Ok(Self::Endpoint),
            0x06 => Ok(Self::DeviceQualifier),
            0x0B => Ok(Self::InterfaceAssociation),
            0x24 => Ok(Self::CSInterface),
            _ => Err(value),
        }
    }
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
struct DeviceDescriptor {
    bcdUSB: u16,
    bDeviceClass: u8,
    bDeviceSubClass: u8,
    bDeviceProtocol: u8,
    bMaxPacketSize0: u8,
    idVendor: u16,
    idProduct: u16,
    bcdDevice: u16,
    manufacturer: Option<StringDescriptorIndex>,
    product: Option<StringDescriptorIndex>,
    serial_number: Option<StringDescriptorIndex>,
    bNumConfigurations: u8,
}

macro_rules! write_fn {
    () => {
        const fn try_write(&self, output: &mut [u8]) -> Result<(), ()> {
            let len = self.length() as usize;
            output[0] = self.length();
            output[1] = self.descriptor_type();
            if output.len() < len - 2 {
                return Err(());
            }
            copy_const(output, 2..len, self.as_slice());
            Ok(())
        }
    };
}

macro_rules! write_fn_const {
    () => {
        const LENGTH_USIZE: usize = Self::LENGTH as usize;

        const fn length(&self) -> u8 {
            Self::LENGTH
        }

        const fn descriptor_type(&self) -> u8 {
            Self::DESCRIPTOR_TYPE as u8
        }

        const fn as_slice(&self) -> &[u8] {
            unsafe {
                core::slice::from_raw_parts(core::ptr::from_ref(self).cast(), size_of::<Self>())
            }
        }

        const fn write(&self, output: &mut [u8; Self::LENGTH_USIZE]) {
            const {
                if size_of::<Self>() != Self::LENGTH_USIZE - 2 {
                    panic!()
                }
            }
            output[0] = self.length();
            output[1] = self.descriptor_type();
            let bytes: [u8; size_of::<Self>()] = unsafe { core::mem::transmute(*self) };
            let len = output.len();
            copy_const(output, 2..len, &bytes);
        }

        write_fn!();
    };
}

impl DeviceDescriptor {
    const LENGTH: u8 = 18;
    const DESCRIPTOR_TYPE: DescriptorType = DescriptorType::Device;

    write_fn_const!();
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
struct InterfaceDescriptor {
    bInterfaceNumber: u8,
    bAlternateSetting: u8,
    bNumEndpoints: u8,
    bInterfaceClass: u8,
    bInterfaceSubClass: u8,
    bInterfaceProtocol: u8,
    iInterface: u8,
}

impl InterfaceDescriptor {
    const LENGTH: u8 = 9;
    const DESCRIPTOR_TYPE: DescriptorType = DescriptorType::Interface;

    write_fn_const!();
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
struct InterfaceAssociationDescriptor {
    bFirstInterface: u8,
    bInterfaceCount: u8,
    bFunctionClass: u8,
    bFunctionSubClass: u8,
    bFunctionProtocol: u8,
    iFunction: u8,
}

impl InterfaceAssociationDescriptor {
    const LENGTH: u8 = 8;
    const DESCRIPTOR_TYPE: DescriptorType = DescriptorType::InterfaceAssociation;

    write_fn_const!();
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
struct ConfigurationDescriptor {
    wTotalLength: u16,
    bNumInterfaces: u8,
    bConfigurationValue: u8,
    iConfiguration: u8,
    bmAttributes: u8,
    bMaxPower: u8,
}

impl ConfigurationDescriptor {
    const LENGTH: u8 = 9;
    const DESCRIPTOR_TYPE: DescriptorType = DescriptorType::Config;

    write_fn_const!();
}

const USB_DIR_IN: u8 = 0x80;
const USB_DIR_OUT: u8 = 0x00;

#[repr(u8)]
#[derive(Copy, Clone)]
enum TransferType {
    Control = 0x0,
    Isochronous = 0x01,
    Bulk = 0x2,
    Interrupt = 0x3,
}

#[derive(Copy, Clone)]
pub enum EndpointId {
    Endpoint0 = 0,
    Endpoint1 = 1,
    Endpoint2 = 2,
    Endpoint3 = 3,
    Endpoint4 = 4,
    Endpoint5 = 5,
    Endpoint6 = 6,
    Endpoint7 = 7,
    Endpoint8 = 8,
    Endpoint9 = 9,
    Endpoint10 = 10,
    Endpoint11 = 11,
    Endpoint12 = 12,
    Endpoint13 = 13,
    Endpoint14 = 14,
    Endpoint15 = 15,
}

impl EndpointId {
    pub const fn new(id: u8) -> Option<Self> {
        match id {
            0 => Some(Self::Endpoint0),
            1 => Some(Self::Endpoint1),
            2 => Some(Self::Endpoint2),
            3 => Some(Self::Endpoint3),
            4 => Some(Self::Endpoint4),
            5 => Some(Self::Endpoint5),
            6 => Some(Self::Endpoint6),
            7 => Some(Self::Endpoint7),
            8 => Some(Self::Endpoint8),
            9 => Some(Self::Endpoint9),
            10 => Some(Self::Endpoint10),
            11 => Some(Self::Endpoint11),
            12 => Some(Self::Endpoint12),
            13 => Some(Self::Endpoint13),
            14 => Some(Self::Endpoint14),
            15 => Some(Self::Endpoint15),
            _ => None,
        }
    }
    pub const fn is_zero(self) -> bool {
        matches!(self, Self::Endpoint0)
    }
}

pub const fn endpoint_ctrl_register(
    endpoint: EndpointId,
    direction: Direction,
) -> Option<DPRAMPtr<u32>> {
    const BASE: DPRAMPtr<u32> = DPRAMPtr::new(0);
    let offset = match direction {
        Direction::Out => 1,
        Direction::In => 0,
    };
    match endpoint.is_zero() {
        true => None,
        false => Some(BASE.offset((endpoint as u8 * 2 + offset) as _)),
    }
}

pub const fn endpoint_buffer_ctrl_register(
    endpoint: EndpointId,
    direction: Direction,
) -> DPRAMPtr<u32> {
    const BASE: DPRAMPtr<u32> = DPRAMPtr::new(0x80);
    let offset = match direction {
        Direction::Out => 1,
        Direction::In => 0,
    };
    BASE.offset((endpoint as u8 * 2 + offset) as _)
}

pub const fn endpoint_buffer(endpoint: EndpointId, direction: Direction) -> DPRAMPtr<[u8; 64]> {
    const BASE: DPRAMPtr<[u8; 64]> = DPRAMPtr::new(0x100);
    if matches!(endpoint, EndpointId::Endpoint0) {
        return BASE;
    }
    let offset = match direction {
        Direction::Out => 1,
        Direction::In => 0,
    };
    BASE.offset((endpoint as u8 * 2 + offset) as _)
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
struct EndpointDescriptor {
    endpoint_address: u8,
    attributes: TransferType,
    max_packet_size: u16,
    interval: u8,
}

impl EndpointDescriptor {
    const LENGTH: u8 = 7;
    const DESCRIPTOR_TYPE: DescriptorType = DescriptorType::Endpoint;

    write_fn_const!();

    const fn endpoint(self) -> EndpointId {
        EndpointId::new(self.endpoint_address & 0x7F).expect("id should be within bounds")
    }

    fn direction(self) -> Direction {
        (self.endpoint_address & 0x80)
            .try_into()
            .expect("unreachable")
    }
}

#[derive(Clone)]
struct DeviceConfig<'a> {
    device_descriptor: &'a DeviceDescriptor,
    configuration_descriptor: &'a ConfigurationDescriptor,
    lang_descriptor: [u8; 4],
    descriptor_strings: &'a [&'a str],
    endpoints: &'a [EndpointDescriptor],
}

impl DeviceConfig<'_> {
    fn configure_endpoints(&self) {
        assert!(self.endpoints.len() < 16);
        for endpoint in self.endpoints {
            let id = EndpointId::new(endpoint.endpoint_address & 0x7F)
                .expect("Should not have more than 16 endpoints");
            let dir = Direction::try_from(endpoint.endpoint_address & 0x80)
                .expect("Direction is either 0x00 or 0x80");
            let dpram_offset = endpoint_buffer(id, dir).byte_offset;
            const TRANSFER_TYPE_BITS: u32 = 26;
            const INTERRUPT_EVERY_2_BUFFERS: u32 = 1 << 28;
            const INTERRUPT_EVERY_BUFFER: u32 = 1 << 29;
            const DOUBLE_BUFFERED: u32 = 1 << 30;
            const ENABLE: u32 = 1 << 31;
            if let Some(register) = endpoint_ctrl_register(id, dir) {
                register.write(
                    ENABLE
                        | (DOUBLE_BUFFERED * 0)
                        | INTERRUPT_EVERY_BUFFER
                        | (((endpoint.attributes as u8 & 0b11) as u32) << TRANSFER_TYPE_BITS)
                        | dpram_offset as u32,
                );
            }
        }
    }
}

mod cdc {
    use crate::{common::copy_const, usb::DescriptorType};

    #[repr(C, packed)]
    #[derive(Clone, Copy)]
    pub struct CallManagement {
        pub bmCapabilities: u8,
        pub data_interface: u8,
    }

    #[repr(C, packed)]
    #[derive(Clone, Copy)]
    pub struct AbstractControlManagement {
        pub bmCapabilities: u8,
    }

    #[repr(C, packed)]
    #[derive(Clone, Copy)]
    pub struct Union {
        pub control_interface: u8,
        pub subordinate_interface: u8,
    }

    #[derive(Clone, Copy)]
    pub enum CommunicationsDescriptor {
        Header,
        CallManagement(CallManagement),
        AbstractControlManagement(AbstractControlManagement),
        Union(Union),
    }

    impl CommunicationsDescriptor {
        pub const fn length(&self) -> u8 {
            match self {
                Self::Header => 5,
                Self::CallManagement(_) => 5,
                Self::AbstractControlManagement(_) => 4,
                Self::Union(_) => 5,
            }
        }

        pub const fn descriptor_type(&self) -> u8 {
            DescriptorType::CSInterface as u8
        }

        pub const fn descriptor_subtype(&self) -> u8 {
            match self {
                Self::Header => 0x00,
                Self::CallManagement(_) => 0x01,
                Self::AbstractControlManagement(_) => 0x02,
                Self::Union(_) => 0x06,
            }
        }

        pub const fn as_slice(&self) -> &[u8] {
            macro_rules! to_slice {
                ($name: ident, $type: ident) => {
                    unsafe {
                        core::slice::from_raw_parts(
                            core::ptr::from_ref($name).cast(),
                            size_of::<$type>(),
                        )
                    }
                };
            }
            match self {
                Self::Header => const { 0x0120u16.to_le_bytes() }.as_slice(),
                Self::CallManagement(cm) => to_slice!(cm, CallManagement),
                Self::AbstractControlManagement(acm) => to_slice!(acm, AbstractControlManagement),
                Self::Union(union) => to_slice!(union, Union),
            }
        }

        pub const fn try_write(&self, output: &mut &mut [u8]) -> Result<(), ()> {
            let len = self.length() as usize;
            if output.len() < len {
                return Err(());
            }
            let slice = core::mem::replace(output, &mut []);
            let (start, end) = slice.split_at_mut(len);
            *output = end;
            start[0] = self.length();
            start[1] = self.descriptor_type();
            start[2] = self.descriptor_subtype();
            copy_const(start, 3..len, self.as_slice());
            Ok(())
        }
    }
}
