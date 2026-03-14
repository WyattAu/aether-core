#!/bin/bash
set -e

# Wait for KVM device
if [ ! -e /dev/kvm ]; then
    echo "Warning: KVM not available, OCI actors will not work"
fi

# Wait for io_uring support
if [ ! -e /proc/sys/kernel/io_uring_disabled ]; then
    echo "Warning: io_uring may not be available"
fi

# Start Aether
exec aether "$@"
