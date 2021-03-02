#include <linux/bug.h>
#include <linux/printk.h>
#include <linux/uaccess.h>
#include <linux/version.h>
#include <linux/poll.h>

void bug_helper(void)
{
    BUG();
}

void print_len(size_t n)
{
    printk(KERN_NOTICE "Rust: allocated length is: %lu\n", n);
}

void init_waitqueue_head_helper(wait_queue_head_t* h)
{
    init_waitqueue_head(h);
}

void wake_up_interruptible_helper(wait_queue_head_t* h)
{
    wake_up_interruptible(h);
}

void poll_wait_helper(struct file* f, wait_queue_head_t* h, poll_table* t)
{
    poll_wait(f, h, t);
}

int access_ok_helper(const void __user *addr, unsigned long n)
{
#if defined(OS_CENTOS) && LINUX_VERSION_CODE >= KERNEL_VERSION(4, 18, 0)
    return access_ok(addr, n);
#elif LINUX_VERSION_CODE >= KERNEL_VERSION(5, 0, 0) /* v5.0-rc1~46 */
    return access_ok(addr, n);
#else
    return access_ok(0, addr, n);
#endif
}

/* see https://github.com/rust-lang/rust-bindgen/issues/1671 */
_Static_assert(__builtin_types_compatible_p(size_t, uintptr_t),
               "size_t must match uintptr_t, what architecture is this??");
