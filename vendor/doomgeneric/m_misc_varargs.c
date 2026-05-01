// Variadic helper functions that cannot be ported to stable Rust.
// These are the only functions from m_misc.c that remain in C.

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdarg.h>
#include "m_misc.h"

char *M_StringJoin(const char *s, ...)
{
    char *result;
    const char *v;
    va_list args;
    size_t result_len;

    result_len = strlen(s) + 1;

    va_start(args, s);
    for (;;) {
        v = va_arg(args, const char *);
        if (v == NULL) break;
        result_len += strlen(v);
    }
    va_end(args);

    result = malloc(result_len);
    if (result == NULL) {
        I_Error("M_StringJoin: Failed to allocate new string.");
        return NULL;
    }

    M_StringCopy(result, s, result_len);

    va_start(args, s);
    for (;;) {
        v = va_arg(args, const char *);
        if (v == NULL) break;
        M_StringConcat(result, v, result_len);
    }
    va_end(args);

    return result;
}

int M_vsnprintf(char *buf, size_t buf_len, const char *s, va_list args)
{
    int result;

    if (buf_len < 1) return 0;

    result = vsnprintf(buf, buf_len, s, args);

    if (result < 0 || result >= (int)buf_len) {
        buf[buf_len - 1] = '\0';
        result = (int)buf_len - 1;
    }

    return result;
}

int M_snprintf(char *buf, size_t buf_len, const char *s, ...)
{
    va_list args;
    int result;
    va_start(args, s);
    result = M_vsnprintf(buf, buf_len, s, args);
    va_end(args);
    return result;
}
