#pragma once
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

void overlay_show(const char *state, double x, double y, double width, double height);
void overlay_hide(void);
void overlay_set_levels(const float *levels, size_t count);
void overlay_set_stream_text(const char *committed, const char *tentative, const char *phase, const char *workKind);

#ifdef __cplusplus
}
#endif
