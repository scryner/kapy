#ifndef __STREAM_H__

#define __STREAM_H__

#include <glib.h>
#include <gexiv2/gexiv2.h>

// buf stream
struct _BufStream{
    size_t curr;
    unsigned char *buf;
    size_t length;
    size_t capacity;
    int moved;
    int incr_count;
};

typedef struct _BufStream BufStream;

// stream callback
ManagedStreamCallbacks* buf_stream_new(unsigned char *buf, size_t buf_len);
ManagedStreamCallbacks *buf_stream_new_empty(size_t initial_size);
void buf_stream_free(ManagedStreamCallbacks **cb);
unsigned char *buf_stream_get_data(ManagedStreamCallbacks *cb, size_t *length);

gboolean buf_stream_can_seek(void *handle);
gboolean buf_stream_can_read(void *handle);
gboolean buf_stream_can_write(void *handle);
gint64 buf_stream_length(void *handle);
gint64 buf_stream_position(void *handle);
gint32 buf_stream_read(void *handle, void *buffer, gint32 offset, gint32 count);
void buf_stream_write(void *handle, void *buffer, gint32 offset, gint32 count);
void buf_stream_seek(void *handle, gint64 offset, WrapperSeekOrigin origin);
void buf_stream_flush(void *handle);

#endif