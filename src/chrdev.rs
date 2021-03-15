use core::convert::TryInto;
use core::mem;
use core::ops::Range;

use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;

use crate::bindings;
use crate::c_types;
use crate::error::{Error, KernelResult};
use crate::file_operations;
use crate::types::CStr;
use alloc::format;
use crate::alloc::borrow::ToOwned;

pub fn builder(name: CStr<'static>, minors: Range<u16>) -> KernelResult<Builder> {
    Ok(Builder {
        name,
        minors,
        file_ops: vec![],
        sys_class: core::ptr::null_mut(),
        parent_dev: core::ptr::null_mut()
    })
}

pub struct Builder {
    name: CStr<'static>,
    minors: Range<u16>,
    file_ops: Vec<&'static bindings::file_operations>,
    sys_class: *mut bindings::class,
    parent_dev: *mut bindings::device,
}

impl Builder {
    pub fn register_device<T: file_operations::FileOperations>(mut self) -> Builder {
        if self.file_ops.len() >= self.minors.len() {
            panic!("More devices registered than minor numbers allocated.")
        }
        self.file_ops
            .push(&file_operations::FileOperationsVtable::<T>::VTABLE);
        self
    }

    pub fn register_class(
        mut self,
        sys_class: &mut bindings::class,
        parent_dev: &mut bindings::device,
    ) -> Builder {
        self.parent_dev = parent_dev;
        self.sys_class = sys_class;
        self
    }

    pub fn build(self) -> KernelResult<Registration> {
        let mut dev: bindings::dev_t = 0;
        let res = unsafe {
            bindings::alloc_chrdev_region(
                &mut dev,
                self.minors.start.into(),
                self.minors.len().try_into()?,
                self.name.as_ptr() as *const c_types::c_char,
            )
        };
        if res != 0 {
            return Err(Error::from_kernel_errno(res));
        }

        let mut name_clear = self.name.to_owned();
        name_clear.truncate(name_clear.len() - 1);

        // Turn this into a boxed slice immediately because the kernel stores pointers into it, and
        // so that data should never be moved.
        let mut cdevs = vec![unsafe { mem::zeroed() }; self.file_ops.len()].into_boxed_slice();
        for (i, file_op) in self.file_ops.iter().enumerate() {
            unsafe {
                bindings::cdev_init(&mut cdevs[i], *file_op);
                cdevs[i].owner = &mut bindings::__this_module;
                let rc = bindings::cdev_add(&mut cdevs[i], dev + i as bindings::dev_t, 1);
                if rc != 0 {
                    // Clean up the ones that were allocated.
                    for j in 0..=i {
                        bindings::cdev_del(&mut cdevs[j]);
                    }
                    bindings::unregister_chrdev_region(dev, self.minors.len() as _);
                    return Err(Error::from_kernel_errno(rc));
                }

                let device = 
                    bindings::device_create(
                        self.sys_class,
                        self.parent_dev,
                        dev + i as bindings::dev_t,
                        core::ptr::null_mut(),
                        format!("{}_{}\x00", name_clear, i).as_ptr() as *const c_types::c_char
                    );
                if device.is_null() {
                    for j in 0..=i {
                        bindings::cdev_del(&mut cdevs[j]);
                    }
                    bindings::unregister_chrdev_region(dev, self.minors.len() as _);
                    return Err(Error::EFAULT);
                }
            }
        }

        Ok(Registration {
            dev,
            count: self.minors.len(),
            cdevs,
            sys_class: self.sys_class,
        })
    }
}

pub struct Registration {
    dev: bindings::dev_t,
    count: usize,
    cdevs: Box<[bindings::cdev]>,
    sys_class: *mut bindings::class,
}

// This is safe because Registration doesn't actually expose any methods.
unsafe impl Sync for Registration {}

impl Drop for Registration {
    fn drop(&mut self) {
        unsafe {
            bindings::device_destroy(self.sys_class, self.dev);
            for dev in self.cdevs.iter_mut() {
                bindings::cdev_del(dev);
            }
            bindings::unregister_chrdev_region(self.dev, self.count as _);
        }
    }
}
