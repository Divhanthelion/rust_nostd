/* ffi2 harness: C owns the memory, Rust owns the logic.
 * (The checker compiles its own pristine copy of this file.) */
#include <stdalign.h>
#include <stdint.h>
#include <stddef.h>
#include <stdio.h>

typedef struct Filter Filter;              /* opaque: C never sees inside */
extern const size_t NOSTD_FILTER_SIZE;
extern const size_t NOSTD_FILTER_ALIGN;

Filter *nostd_filter_init(void *storage, size_t storage_len, uint8_t window);
int32_t nostd_filter_push(Filter *f, int32_t sample, int32_t *out_avg);
typedef void (*visit_fn)(void *user, size_t index, int32_t value);
int32_t nostd_filter_for_each(const Filter *f, visit_fn cb, void *user);

__attribute__((weak)) void rust_eh_personality(void) {}

struct Totals {
    int64_t sum;
    size_t visits;
};

static void add_up(void *user, size_t index, int32_t value) {
    struct Totals *t = (struct Totals *)user;
    t->sum += value;
    t->visits += 1;
    printf("  visit %zu: %d\n", index, value);
}

int main(void) {
    printf("size ok: %d, align ok: %d\n", NOSTD_FILTER_SIZE <= 128, NOSTD_FILTER_ALIGN <= 8);

    alignas(8) static unsigned char storage[128];
    Filter *f = nostd_filter_init(storage, sizeof storage, 3);
    printf("init: %s\n", f ? "ok" : "NULL");
    int32_t samples[] = {10, 20, 30, 40, -100};
    for (int i = 0; i < 5; i++) {
        int32_t avg = 0;
        int32_t rc = nostd_filter_push(f, samples[i], &avg);
        printf("push %d -> rc=%d avg=%d\n", samples[i], rc, avg);
    }
    struct Totals t = {0, 0};
    int32_t n = nostd_filter_for_each(f, add_up, &t);
    printf("for_each -> %d (sum=%lld, visits=%zu)\n", n, (long long)t.sum, t.visits);

    printf("init(NULL) = %s\n", nostd_filter_init(NULL, 128, 3) ? "ptr" : "NULL");
    printf("init(too small) = %s\n", nostd_filter_init(storage, 4, 3) ? "ptr" : "NULL");
    printf("init(misaligned) = %s\n", nostd_filter_init(storage + 1, 120, 3) ? "ptr" : "NULL");
    printf("init(window 0) = %s\n", nostd_filter_init(storage, 128, 0) ? "ptr" : "NULL");
    printf("init(window 9) = %s\n", nostd_filter_init(storage, 128, 9) ? "ptr" : "NULL");
    printf("push(NULL) = %d\n", nostd_filter_push(NULL, 1, NULL));
    printf("for_each(NULL cb) = %d\n", nostd_filter_for_each(f, NULL, NULL));
    return 0;
}
