#include <stdio.h>
#include <stdlib.h>
#include "stream.h"

#define BUF_INCR_RATE 1.2

static void s_buf_stream_incr(BufStream *stream);
static BufStream *s_buf_stream_new(unsigned char *buf, size_t buf_len);
static void s_buf_stream_free(BufStream *stream);

ManagedStreamCallbacks *buf_stream_new(unsigned char *buf, size_t buf_len) {
    ManagedStreamCallbacks *cb = NULL;
    BufStream *stream = NULL;

    cb = (ManagedStreamCallbacks*)malloc(sizeof(ManagedStreamCallbacks));
    memset(cb, 0, sizeof(ManagedStreamCallbacks));

    stream = s_buf_stream_new(buf, buf_len);

    // set callbacks
    cb->handle = (void*) stream;
    cb->CanSeek = buf_stream_can_seek;
    cb->CanRead = buf_stream_can_read;
    cb->CanWrite = buf_stream_can_write;
    cb->Length = buf_stream_length;
    cb->Position = buf_stream_position;
    cb->Read = buf_stream_read;
    cb->Write = buf_stream_write;
    cb->Seek = buf_stream_seek;
    cb->Flush = buf_stream_flush;

    return cb;
}

ManagedStreamCallbacks *buf_stream_new_empty(size_t initial_size) {
    unsigned char *buf = (unsigned char*) malloc(sizeof(unsigned char) * initial_size);

    ManagedStreamCallbacks *cb = NULL;
    BufStream *stream = NULL;

    cb = (ManagedStreamCallbacks*)malloc(sizeof(ManagedStreamCallbacks));
    memset(cb, 0, sizeof(ManagedStreamCallbacks));

    stream = (BufStream*) malloc(sizeof(BufStream));
    memset(stream, 0, sizeof(BufStream));
    stream->buf = buf;
    stream->capacity = initial_size;
    stream->length = 0;

    // set callbacks
    cb->handle = (void*) stream;
    cb->CanSeek = buf_stream_can_seek;
    cb->CanRead = buf_stream_can_read;
    cb->CanWrite = buf_stream_can_write;
    cb->Length = buf_stream_length;
    cb->Position = buf_stream_position;
    cb->Read = buf_stream_read;
    cb->Write = buf_stream_write;
    cb->Seek = buf_stream_seek;
    cb->Flush = buf_stream_flush;

    return cb;
}

void buf_stream_free(ManagedStreamCallbacks **cb) {
    if (*cb == NULL) {
        return;
    }

    if ((*cb)->handle != NULL) {
        s_buf_stream_free((*cb)->handle);
        free((*cb)->handle);
    }

    free(*cb);
    *cb = NULL;
}


gboolean buf_stream_can_seek(void *handle) {
    (void) handle;
    return TRUE;
}

gboolean buf_stream_can_read(void *handle) {
    (void) handle;
    return TRUE;
}

gboolean buf_stream_can_write(void *handle) {
    (void) handle;
    return TRUE;
}

gint64 buf_stream_length(void *handle) {
    BufStream *stream = (BufStream*) handle;
    return (gint64) stream->length;
}

gint64 buf_stream_position(void *handle) {
    BufStream *stream = (BufStream*) handle;
    return (gint64) stream->curr;
}


gint32 buf_stream_read(void *handle, void *buffer, gint32 offset, gint32 count) {
    BufStream *stream = (BufStream*) handle;
    gint32 copying = -1;
    size_t remained = 0;

    if (stream->curr >= stream->length) {
        return 0;   // EOF
    }

    // calculate count to copying
    remained = stream->length - stream->curr;
    if ((gint32)remained >= count) {
        copying = count;
    } else {
        copying = (gint32) remained;
    }

    // copy to buffer
    memcpy(buffer + (size_t) offset, stream->buf + stream->curr, copying);
    stream->curr += (size_t) copying;

    return copying;
}

void buf_stream_write(void *handle, void *buffer, gint32 offset, gint32 count) {
    BufStream *stream = (BufStream*) handle;
    size_t remained = 0;
    size_t new_len = 0;

    // increase buffer size if it needed
    do {
        remained = stream->capacity - stream->curr;
        if ((size_t) count > remained) {
            s_buf_stream_incr(stream);
        } else {
            break;
        }
    } while(1);

    new_len = stream->curr + (size_t) count;

    // copying to stream buffer
    memcpy(stream->buf + stream->curr, buffer + (size_t) offset, (size_t) count);
    stream->length = new_len;
    stream->curr += (size_t) count;
}

void buf_stream_seek(void *handle, gint64 offset, WrapperSeekOrigin origin) {
    BufStream *stream = (BufStream*) handle;

    switch (origin) {
        case Begin:
            stream->curr = (size_t) offset;
            break;

        case Current:
            stream->curr += (size_t) offset;
            break;

        case End:
            stream->curr -= (size_t) offset;
            break;
    }
}

void buf_stream_flush(void *handle) {
    (void) handle;
    // noop
}

unsigned char *buf_stream_get_data(ManagedStreamCallbacks *cb, size_t *length) {
    BufStream *stream = (BufStream*) cb->handle;
    stream->moved = 1;

    *length = stream->length;
    return stream->buf;
}

static void s_buf_stream_incr(BufStream *stream) {
    size_t length = stream->length;

    unsigned char *old_buf = stream->buf;
    size_t old_capacity = stream->capacity;
    size_t new_capacity = (size_t)((double)old_capacity * BUF_INCR_RATE);

    unsigned char *new_buf = (unsigned char*) malloc(new_capacity);
    memcpy(new_buf, old_buf, length);
    memset(new_buf + length, 0, new_capacity - length);

    stream->buf = new_buf;
    stream->capacity = new_capacity;

    if (stream->incr_count > 0) {
        free(old_buf);
    }

    stream->incr_count += 1;
}

static BufStream *s_buf_stream_new(unsigned char *buf, size_t buf_len) {
    BufStream *stream = NULL;
    unsigned char* new_buf = NULL;

    stream = (BufStream*) malloc(sizeof(BufStream));
    new_buf = (unsigned char*) malloc(buf_len);
    memcpy(new_buf, buf, buf_len);

    stream->buf = buf;
    stream->length = buf_len;
    stream->capacity = buf_len;

    return stream;
}

static void s_buf_stream_free(BufStream *stream) {
    if (stream->buf != NULL && stream->incr_count > 0 && stream->moved != 1) {
        free(stream->buf);
    }
}
