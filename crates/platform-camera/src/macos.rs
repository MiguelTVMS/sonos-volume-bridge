//! Read-only CoreMediaIO properties; no capture sessions or camera content.
#![allow(unsafe_code)] // Narrow FFI boundary, matching platform-audio's native adapters.
use super::{
    observer::{CameraSource, Observer},
    worker::{Monitor, unavailable},
};
use sonos_volume_bridge_integration::camera::CameraPort;
use std::{
    ffi::c_void,
    sync::{Arc, Mutex, mpsc},
    time::{Duration, Instant},
};

#[repr(C)]
#[derive(Clone, Copy)]
struct Address {
    selector: u32,
    scope: u32,
    element: u32,
}
const fn address(selector: [u8; 4]) -> Address {
    Address {
        selector: u32::from_be_bytes(selector),
        scope: u32::from_be_bytes(*b"glob"),
        element: 0,
    }
}
const DEVICES: Address = address(*b"dev#");
const RUNNING: Address = address(*b"gone");
const STREAMS: Address = Address {
    selector: u32::from_be_bytes(*b"stm#"),
    scope: u32::from_be_bytes(*b"inpt"),
    element: 0,
};
type Listener = unsafe extern "C" fn(u32, u32, *const Address, *mut c_void) -> i32;
#[link(name = "CoreMediaIO", kind = "framework")]
unsafe extern "C" {
    fn CMIOObjectGetPropertyDataSize(
        object: u32,
        address: *const Address,
        qualifier_size: u32,
        qualifier: *const c_void,
        size: *mut u32,
    ) -> i32;
    fn CMIOObjectGetPropertyData(
        object: u32,
        address: *const Address,
        qualifier_size: u32,
        qualifier: *const c_void,
        size: u32,
        used: *mut u32,
        data: *mut c_void,
    ) -> i32;
    fn CMIOObjectAddPropertyListener(
        object: u32,
        address: *const Address,
        listener: Listener,
        data: *mut c_void,
    ) -> i32;
    fn CMIOObjectRemovePropertyListener(
        object: u32,
        address: *const Address,
        listener: Listener,
        data: *mut c_void,
    ) -> i32;
}
// Callback owns no user pointer, so late native delivery cannot dereference freed state.
static CHANGED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);
unsafe extern "C" fn changed(_: u32, _: u32, _: *const Address, _: *mut c_void) -> i32 {
    CHANGED.store(true, std::sync::atomic::Ordering::Release);
    0
}
fn values(object: u32, property: &Address) -> Option<Vec<u32>> {
    let mut size = 0;
    // SAFETY: pointers reference initialized storage; CoreMediaIO only writes within supplied sizes.
    unsafe {
        if CMIOObjectGetPropertyDataSize(object, property, 0, std::ptr::null(), &raw mut size) != 0
            || size % 4 != 0
            || size > 65536
        {
            return None;
        }
        let mut data = vec![0_u32; (size / 4) as usize];
        let mut used = 0;
        if CMIOObjectGetPropertyData(
            object,
            property,
            0,
            std::ptr::null(),
            size,
            &raw mut used,
            data.as_mut_ptr().cast(),
        ) != 0
            || used > size
            || used % 4 != 0
        {
            return None;
        }
        data.truncate((used / 4) as usize);
        Some(data)
    }
}
struct Listeners(Vec<(u32, Address)>);
impl Listeners {
    fn add(&mut self, object: u32, property: Address) -> bool {
        // SAFETY: static callback and no client data; removed on drop.
        if unsafe {
            CMIOObjectAddPropertyListener(
                object,
                &raw const property,
                changed,
                std::ptr::null_mut(),
            )
        } != 0
        {
            return false;
        }
        self.0.push((object, property));
        true
    }
}
impl Drop for Listeners {
    fn drop(&mut self) {
        for (object, property) in &self.0 {
            // SAFETY: matches each successful registration exactly.
            unsafe {
                CMIOObjectRemovePropertyListener(*object, property, changed, std::ptr::null_mut());
            }
        }
    }
}
struct NativeSource {
    listeners: Listeners,
}
impl CameraSource for NativeSource {
    fn devices(&mut self) -> Option<Vec<u32>> {
        let mut cameras = Vec::new();
        for device in values(1, &DEVICES)? {
            if !values(device, &STREAMS)?.is_empty() {
                cameras.push(device);
            }
        }
        cameras.sort_unstable();
        Some(cameras)
    }
    fn watch(&mut self, devices: &[u32]) -> bool {
        self.clear();
        self.listeners.add(1, DEVICES)
            && devices
                .iter()
                .all(|device| self.listeners.add(*device, RUNNING))
    }
    fn running(&mut self, device: u32) -> Option<bool> {
        match values(device, &RUNNING)?.as_slice() {
            [0] => Some(false),
            [1] => Some(true),
            _ => None,
        }
    }
    fn clear(&mut self) {
        self.listeners = Listeners(Vec::new());
    }
}

pub fn start() -> Box<dyn CameraPort> {
    let state = Arc::new(Mutex::new((unavailable(), Instant::now())));
    let output = Arc::clone(&state);
    let (stop, stopped) = mpsc::channel();
    let thread = std::thread::spawn(move || {
        let mut observer = Observer::new(NativeSource {
            listeners: Listeners(Vec::new()),
        });
        let mut last_scan: Option<Instant> = None;
        loop {
            if last_scan.is_none_or(|t| t.elapsed() >= Duration::from_secs(5))
                || CHANGED.swap(false, std::sync::atomic::Ordering::AcqRel)
            {
                let observation = observer.sample();
                if let Ok(mut current) = output.lock() {
                    *current = (observation, Instant::now());
                }
                last_scan = Some(Instant::now());
            }
            match stopped.recv_timeout(Duration::from_millis(100)) {
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                _ => break,
            }
        }
    });
    Box::new(Monitor {
        state,
        stop,
        thread: Some(thread),
    })
}
