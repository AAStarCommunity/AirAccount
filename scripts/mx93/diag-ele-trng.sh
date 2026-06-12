#!/bin/sh
# diag-ele-trng.sh — RUN ON THE MX93 BOARD (over serial), NOT on the Mac.
# Diagnoses the i.MX93 ELE TRNG state to pinpoint why CreateKey crashes the board.
#
# i.MX93 uses the EdgeLock Enclave (ELE / S401), NOT CAAM. The hardware TRNG must be
# started by U-Boot SPL (ele_start_rng). This script tells us whether the ELE TRNG is
# alive at the KERNEL level (safe to query) before we ever risk an OP-TEE RNG call.
#
# Copy to board and run:  sh /root/diag-ele-trng.sh
set -u
echo "==================== ELE TRNG DIAGNOSTIC ===================="

echo "--- 1. SoC / silicon revision (A0 auto-starts TRNG, A1 needs SPL to start it) ---"
cat /sys/devices/soc0/soc_id 2>/dev/null || echo "(no soc0/soc_id)"
cat /sys/devices/soc0/revision 2>/dev/null || echo "(no soc0/revision)"

echo ""
echo "--- 2. Kernel hwrng: which RNG is active? (expect: ele-trng) ---"
cat /sys/class/misc/hw_random/rng_current 2>/dev/null || echo "(no hw_random/rng_current — no hwrng registered!)"
echo "available:"
cat /sys/class/misc/hw_random/rng_available 2>/dev/null || echo "(none)"

echo ""
echo "--- 3. ELE / MU driver in dmesg ---"
dmesg 2>/dev/null | grep -iE "ele|sentinel|s4mu|hwrng|trng|imx-ele|fsl-ele" | tail -30 || echo "(dmesg unavailable)"

echo ""
echo "--- 4. SAFE kernel-side TRNG read test (5s timeout) ---"
echo "    If this returns bytes -> ELE hardware TRNG is ALIVE (problem is OP-TEE-only)."
echo "    If this HANGS/empty   -> ELE TRNG itself is dead (bootloader/firmware issue)."
if command -v timeout >/dev/null 2>&1; then
  timeout 5 dd if=/dev/hwrng bs=16 count=1 2>/dev/null | od -An -tx1 | head -1 \
    && echo "    >> /dev/hwrng OK (ELE TRNG alive at kernel level)" \
    || echo "    >> /dev/hwrng FAILED or TIMED OUT (ELE TRNG not producing entropy)"
else
  echo "    (no 'timeout' cmd; skipping — would risk hang)"
fi

echo ""
echo "--- 5. /dev/urandom sanity (Linux CSPRNG, always works) ---"
head -c 16 /dev/urandom 2>/dev/null | od -An -tx1 | head -1

echo ""
echo "--- 6. OP-TEE version + last boot panic (look for 'ELE RNG is busy') ---"
dmesg 2>/dev/null | grep -iE "optee|tee" | head -20 || echo "(none)"
echo "    (Full OP-TEE secure-world panic shows on the SERIAL boot log, not dmesg —"
echo "     watch the console during boot for: panic 'ELE RNG is busy')"

echo ""
echo "==================== END DIAGNOSTIC ===================="
echo "Interpretation:"
echo "  step4 OK  + CreateKey crashes -> OP-TEE built with CFG_WITH_SOFTWARE_PRNG=n; rebuild OP-TEE with =y, or fix runtime ELE path"
echo "  step4 HANG                    -> ELE TRNG never started: fix U-Boot SPL ele_start_rng() + reflash imx-boot/ELE firmware"
echo "  step2 'no hwrng'              -> ELE MU driver didn't probe: kernel/devicetree/firmware issue"
