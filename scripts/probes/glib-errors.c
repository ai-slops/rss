#include <errno.h>
#include <glib.h>
#include <stdio.h>
#include <string.h>

int main(void) {
    const char *message = g_strerror(EAGAIN);
    printf("glib-runtime=%u.%u.%u headers=%u.%u.%u\n", glib_major_version, glib_minor_version,
           glib_micro_version, GLIB_MAJOR_VERSION, GLIB_MINOR_VERSION, GLIB_MICRO_VERSION);
    printf("libc-eagain=%s\n", strerror(EAGAIN));
    printf("glib-eagain=%s\n", message == NULL ? "NULL" : message);
    return message == NULL ? 1 : 0;
}
