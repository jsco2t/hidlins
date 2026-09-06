#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

typedef bool (*HidlinsClipboardReceiver)(
    const uint8_t *bytes,
    size_t length,
    void *context
);

int32_t hidlins_ios_consume_clipboard(
    const uint8_t *ticket,
    size_t ticket_length,
    HidlinsClipboardReceiver receiver,
    void *context
);
