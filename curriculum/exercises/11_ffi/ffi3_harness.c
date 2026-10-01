/* ffi3 harness: the "C platform" that your Rust component calls into.
 * (The checker compiles its own pristine copy of this file.) */
#include <stdint.h>
#include <stddef.h>
#include <stdio.h>

__attribute__((weak)) void rust_eh_personality(void) {}

/* --- services the C platform offers to Rust --- */

void c_log(int level, const char *msg) {
    printf("[L%d] %s\n", level, msg);
}

/* Sensor readings in tenths of a degree. Channels 0..2 exist. */
int c_read_sensor(int channel, int32_t *out) {
    static const int32_t values[] = {215, -40, 1000};
    if (channel < 0 || channel > 2 || out == NULL) {
        return -1;
    }
    *out = values[channel];
    return 0;
}

uint32_t c_millis(void) {
    static uint32_t now = 0;
    now += 100;
    return now;
}

/* --- the Rust component --- */
int32_t rust_poll(const int32_t *channels, size_t n);

int main(void) {
    int32_t channels[] = {0, 1, 7, 2};
    int32_t ok = rust_poll(channels, 4);
    printf("rust_poll returned %d\n", ok);
    printf("rust_poll(NULL) returned %d\n", rust_poll(NULL, 4));
    return 0;
}
