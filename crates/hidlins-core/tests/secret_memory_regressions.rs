use hidlins_core::MasterPassword;
use zeroize::Zeroize as _;

#[test]
fn secret_values_preserve_empty_unicode_nul_and_large_inputs() {
    for value in [
        String::new(),
        "Gàidhlig 🔒".into(),
        "embedded\0fixture".into(),
        "λ🌲".repeat(20_000),
    ] {
        let expected = zeroize::Zeroizing::new(value.clone());
        let mut password = MasterPassword::new(value);
        assert!(
            password.as_bytes() == expected.as_bytes(),
            "exact UTF-8 bytes must survive wrapping"
        );
        assert_eq!(format!("{password:?}"), "MasterPassword(***)");
        password.zeroize();
        assert!(password.as_bytes().is_empty());
        assert_eq!(format!("{password:?}"), "MasterPassword(***)");
    }
}

#[test]
fn secret_ownership_remains_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<MasterPassword>();
    let password = MasterPassword::new("thread-fixture".into());
    assert!(
        std::thread::spawn(move || password.as_bytes() == b"thread-fixture")
            .join()
            .unwrap()
    );
}

#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use std::process::Command;

    fn isolated(case: &str, body: impl FnOnce()) {
        const CHILD: &str = "HIDLINS_MEMORY_REGRESSION_CHILD";
        if std::env::var(CHILD).as_deref() == Ok(case) {
            body();
            return;
        }
        let output = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", case, "--nocapture"])
            .env(CHILD, case)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{case}: child failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[allow(unsafe_code)]
    fn page_size() -> usize {
        // SAFETY: sysconf takes no pointers and only queries immutable OS
        // page-size metadata. The result is checked before address arithmetic.
        let result = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
        let size = usize::try_from(result).expect("valid OS page size");
        assert!(size > 0);
        size
    }

    fn locked_kib() -> usize {
        let status = std::fs::read_to_string("/proc/self/status").unwrap();
        status
            .lines()
            .find_map(|line| line.strip_prefix("VmLck:"))
            .expect("kernel exposes locked-memory accounting")
            .split_whitespace()
            .next()
            .unwrap()
            .parse()
            .unwrap()
    }

    fn same_page_inputs() -> (String, String) {
        let page = page_size();
        let mut pool: Vec<_> = (0..256)
            .map(|_| Some("synthetic-page-sharing-fixture".to_owned()))
            .collect();
        for first in 0..pool.len() {
            let a = pool[first].as_ref().unwrap();
            let a_page = a.as_ptr() as usize / page;
            if (a.as_ptr() as usize + a.len() - 1) / page != a_page {
                continue;
            }
            for second in first + 1..pool.len() {
                let b = pool[second].as_ref().unwrap();
                if b.as_ptr() as usize / page == a_page
                    && (b.as_ptr() as usize + b.len() - 1) / page == a_page
                {
                    return (pool[first].take().unwrap(), pool[second].take().unwrap());
                }
            }
        }
        panic!("fixture must establish two ordinary allocator inputs sharing a page");
    }

    #[test]
    fn dropping_one_owner_keeps_the_other_password_locked() {
        isolated(
            "linux::dropping_one_owner_keeps_the_other_password_locked",
            || {
                let (first, second) = same_page_inputs();
                let baseline = locked_kib();
                let first = MasterPassword::new(first);
                assert!(locked_kib() > baseline, "establish that mlock succeeded");
                let second = MasterPassword::new(second);
                assert!(locked_kib() > baseline);
                drop(first);
                assert!(
                    locked_kib() > baseline,
                    "dropping one password unlocked another live password"
                );
                assert_eq!(second.as_bytes(), b"synthetic-page-sharing-fixture");
                drop(second);
                assert_eq!(locked_kib(), baseline);
            },
        );
    }

    #[test]
    fn explicit_zeroize_then_drop_releases_the_original_lock_extent() {
        isolated(
            "linux::explicit_zeroize_then_drop_releases_the_original_lock_extent",
            || {
                let (input, keeper) = same_page_inputs();
                let baseline = locked_kib();
                let mut password = MasterPassword::new(input);
                assert!(locked_kib() > baseline, "establish that mlock succeeded");
                password.zeroize();
                assert!(password.as_bytes().is_empty());
                drop(password);
                // Keep the source page mapped, so freeing the password allocation
                // cannot hide an omitted munlock through an allocator unmap.
                std::hint::black_box(&keeper);
                assert_eq!(
                    locked_kib(),
                    baseline,
                    "zeroize lost the extent needed to release the lock"
                );
            },
        );
    }

    #[test]
    fn zeroize_and_drop_cannot_unlock_another_live_owner() {
        isolated(
            "linux::zeroize_and_drop_cannot_unlock_another_live_owner",
            || {
                let (first, second) = same_page_inputs();
                let baseline = locked_kib();
                let mut first = MasterPassword::new(first);
                let second = MasterPassword::new(second);
                assert!(locked_kib() > baseline, "establish that mlock succeeded");
                first.zeroize();
                drop(first);
                assert!(locked_kib() > baseline);
                assert_eq!(second.as_bytes(), b"synthetic-page-sharing-fixture");
                drop(second);
                assert_eq!(locked_kib(), baseline);
            },
        );
    }

    #[allow(unsafe_code)]
    fn lower_child_lock_quota_to_zero() {
        let mut limit = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        // SAFETY: limit is a live correctly typed output object. This function
        // runs only in the isolated regression child, never the parent runner.
        assert_eq!(
            unsafe { libc::getrlimit(libc::RLIMIT_MEMLOCK, &raw mut limit) },
            0
        );
        limit.rlim_cur = 0;
        // SAFETY: the live input preserves the existing hard limit and only
        // lowers this child process's soft quota; it requests no privilege.
        assert_eq!(
            unsafe { libc::setrlimit(libc::RLIMIT_MEMLOCK, &raw const limit) },
            0
        );
    }

    #[test]
    fn unavailable_lock_quota_preserves_best_effort_secret_behavior() {
        isolated(
            "linux::unavailable_lock_quota_preserves_best_effort_secret_behavior",
            || {
                lower_child_lock_quota_to_zero();
                let baseline = locked_kib();
                let mut password = MasterPassword::new("unlocked-quota-fixture".into());
                assert_eq!(password.as_bytes(), b"unlocked-quota-fixture");
                assert_eq!(locked_kib(), baseline);
                password.zeroize();
                assert!(password.as_bytes().is_empty());
                drop(password);
                assert_eq!(locked_kib(), baseline);
            },
        );
    }
}
