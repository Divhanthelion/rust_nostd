/* ffi1 harness: a C program that calls your Rust staticlib.
 * Read it to see exactly how C expects to use your functions.
 * (The checker compiles its own pristine copy of this file.) */
#include <stdint.h>
#include <stddef.h>
#include <stdio.h>
#include <string.h>

/* The header a tool like cbindgen would generate for your crate. */
struct Stats {
    int16_t min;
    int16_t max;
    int32_t mean;
    uint32_t count;
};

uint32_t nostd_crc32(const uint8_t *data, size_t len);
int32_t nostd_stats(const int16_t *samples, size_t len, struct Stats *out);
size_t nostd_hex(const uint8_t *data, size_t len, char *out, size_t out_len);

/* The host's precompiled Rust `core` may reference this unwinding symbol;
 * panics abort, so it is never called. */
__attribute__((weak)) void rust_eh_personality(void) {}

int main(void) {
    const char *check = "123456789";
    printf("crc32(\"123456789\") = %08x\n", nostd_crc32((const uint8_t *)check, strlen(check)));
    printf("crc32(empty) = %08x\n", nostd_crc32((const uint8_t *)"", 0));
    printf("crc32(NULL, 5) = %08x\n", nostd_crc32(NULL, 5));

    int16_t samples[] = {20, -40, 125, 0, 30, 15};
    struct Stats st = {0, 0, 0, 0};
    int32_t rc = nostd_stats(samples, 6, &st);
    printf("stats rc=%d min=%d max=%d mean=%d count=%u\n", rc, st.min, st.max, st.mean, st.count);
    int16_t neg[] = {-3, -4};
    rc = nostd_stats(neg, 2, &st);
    printf("stats rc=%d min=%d max=%d mean=%d count=%u\n", rc, st.min, st.max, st.mean, st.count);
    printf("stats(NULL samples) = %d\n", nostd_stats(NULL, 3, &st));
    printf("stats(NULL out) = %d\n", nostd_stats(samples, 6, NULL));
    printf("stats(empty) = %d\n", nostd_stats(samples, 0, &st));

    const uint8_t bytes[] = {0xde, 0xad, 0xbe, 0xef};
    char hex[16];
    memset(hex, 'X', sizeof hex);
    size_t n = nostd_hex(bytes, 4, hex, sizeof hex);
    printf("hex = %s (%zu)\n", hex, n);
    n = nostd_hex(bytes, 4, hex, 8); /* needs 9 bytes: 8 digits + NUL */
    printf("hex(small buffer) = %zu\n", n);
    printf("hex(NULL out) = %zu\n", nostd_hex(bytes, 4, NULL, 16));
    return 0;
}
