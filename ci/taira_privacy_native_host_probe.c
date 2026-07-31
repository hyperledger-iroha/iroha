/*
 * Native Linux primitive probe for first-release Taira privacy evidence.
 *
 * This is compiled statically and executed directly by
 * check_taira_privacy_native_host.sh. It verifies the exact kernel mechanisms
 * that the sealed Rust release runner relies on before the expensive proof
 * build starts.
 */

#define _GNU_SOURCE

#include <errno.h>
#include <fcntl.h>
#include <linux/filter.h>
#include <linux/landlock.h>
#include <linux/memfd.h>
#include <linux/openat2.h>
#include <linux/seccomp.h>
#include <pthread.h>
#include <sched.h>
#include <stdatomic.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/prctl.h>
#include <sys/stat.h>
#include <sys/syscall.h>
#include <unistd.h>

#ifndef MFD_EXEC
#define MFD_EXEC 0x0010U
#endif
#ifndef F_SEAL_EXEC
#define F_SEAL_EXEC 0x0020
#endif
#ifndef LANDLOCK_ACCESS_FS_REFER
#define LANDLOCK_ACCESS_FS_REFER (1ULL << 13)
#endif
#ifndef LANDLOCK_ACCESS_FS_TRUNCATE
#define LANDLOCK_ACCESS_FS_TRUNCATE (1ULL << 14)
#endif

static atomic_int worker_ready = 0;
static atomic_int worker_stop = 0;
enum { WORKER_THREAD_COUNT = 5 };

static void *worker(void *unused) {
    (void)unused;
    atomic_fetch_add_explicit(&worker_ready, 1, memory_order_release);
    while (!atomic_load_explicit(&worker_stop, memory_order_acquire)) {
        sched_yield();
    }
    return NULL;
}

static void fail(const char *label) {
    perror(label);
    exit(EXIT_FAILURE);
}

int main(void) {
    const uint64_t handled_access =
        LANDLOCK_ACCESS_FS_EXECUTE |
        LANDLOCK_ACCESS_FS_WRITE_FILE |
        LANDLOCK_ACCESS_FS_READ_FILE |
        LANDLOCK_ACCESS_FS_READ_DIR |
        LANDLOCK_ACCESS_FS_REMOVE_DIR |
        LANDLOCK_ACCESS_FS_REMOVE_FILE |
        LANDLOCK_ACCESS_FS_MAKE_CHAR |
        LANDLOCK_ACCESS_FS_MAKE_DIR |
        LANDLOCK_ACCESS_FS_MAKE_REG |
        LANDLOCK_ACCESS_FS_MAKE_SOCK |
        LANDLOCK_ACCESS_FS_MAKE_FIFO |
        LANDLOCK_ACCESS_FS_MAKE_BLOCK |
        LANDLOCK_ACCESS_FS_MAKE_SYM |
        LANDLOCK_ACCESS_FS_REFER |
        LANDLOCK_ACCESS_FS_TRUNCATE;
    long landlock_abi = syscall(
        SYS_landlock_create_ruleset,
        NULL,
        0,
        LANDLOCK_CREATE_RULESET_VERSION
    );
    if (landlock_abi < 3) {
        errno = landlock_abi < 0 ? errno : ENOTSUP;
        fail("Landlock ABI >= 3");
    }

    int root = open("/", O_PATH | O_DIRECTORY | O_CLOEXEC);
    if (root < 0) {
        fail("open root anchor");
    }
    struct open_how how = {
        .flags = O_RDONLY | O_CLOEXEC,
        .resolve = RESOLVE_BENEATH | RESOLVE_NO_MAGICLINKS | RESOLVE_NO_SYMLINKS,
    };
    int status = (int)syscall(
        SYS_openat2,
        root,
        "proc/version",
        &how,
        sizeof(how)
    );
    if (status < 0) {
        fail("anchored openat2");
    }
    if (close(status) != 0) {
        fail("close openat2 status descriptor");
    }
    if (close(root) != 0) {
        fail("close openat2 root descriptor");
    }

    int memfd = (int)syscall(
        SYS_memfd_create,
        "taira-privacy-host-probe",
        MFD_CLOEXEC | MFD_ALLOW_SEALING | MFD_EXEC
    );
    if (memfd < 0) {
        fail("memfd_create MFD_EXEC");
    }
    const uint8_t byte = 0;
    if (write(memfd, &byte, sizeof(byte)) != (ssize_t)sizeof(byte)) {
        fail("write executable memfd");
    }
    if (fchmod(memfd, 0500) != 0) {
        fail("fchmod executable memfd");
    }
    const int required_seals =
        F_SEAL_WRITE | F_SEAL_GROW | F_SEAL_SHRINK | F_SEAL_EXEC | F_SEAL_SEAL;
    if (fcntl(memfd, F_ADD_SEALS, required_seals) != 0) {
        fail("F_ADD_SEALS including F_SEAL_EXEC");
    }
    int observed_seals = fcntl(memfd, F_GET_SEALS);
    if (observed_seals < 0 || (observed_seals & required_seals) != required_seals) {
        errno = ENOTSUP;
        fail("F_GET_SEALS");
    }
    if (close(memfd) != 0) {
        fail("close executable memfd");
    }

    pthread_t threads[WORKER_THREAD_COUNT];
    for (size_t index = 0; index < WORKER_THREAD_COUNT; ++index) {
        int pthread_error = pthread_create(&threads[index], NULL, worker, NULL);
        if (pthread_error != 0) {
            errno = pthread_error;
            fail("pthread_create");
        }
    }
    while (
        atomic_load_explicit(&worker_ready, memory_order_acquire) !=
        WORKER_THREAD_COUNT
    ) {
        sched_yield();
    }
    if (prctl(PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) != 0) {
        fail("PR_SET_NO_NEW_PRIVS");
    }
    struct sock_filter filter[] = {
        BPF_STMT(BPF_RET | BPF_K, SECCOMP_RET_ALLOW),
    };
    struct sock_fprog program = {
        .len = (unsigned short)(sizeof(filter) / sizeof(filter[0])),
        .filter = filter,
    };
    long installed = syscall(
        SYS_seccomp,
        SECCOMP_SET_MODE_FILTER,
        SECCOMP_FILTER_FLAG_TSYNC,
        &program
    );
    if (installed != 0) {
        errno = installed < 0 ? errno : EBUSY;
        fail("seccomp TSYNC");
    }
    atomic_store_explicit(&worker_stop, 1, memory_order_release);
    for (size_t index = 0; index < WORKER_THREAD_COUNT; ++index) {
        int pthread_error = pthread_join(threads[index], NULL);
        if (pthread_error != 0) {
            errno = pthread_error;
            fail("pthread_join");
        }
    }

    struct landlock_ruleset_attr landlock_attributes = {
        .handled_access_fs = handled_access,
    };
    int ruleset = (int)syscall(
        SYS_landlock_create_ruleset,
        &landlock_attributes,
        sizeof(landlock_attributes),
        0
    );
    if (ruleset < 0) {
        fail("create deny-all Landlock ruleset");
    }
    if (syscall(SYS_landlock_restrict_self, ruleset, 0) != 0) {
        fail("enter deny-all Landlock domain");
    }
    if (close(ruleset) != 0) {
        fail("close Landlock ruleset");
    }

    printf("%ld\n", landlock_abi);
    return EXIT_SUCCESS;
}
