#include "msg_buf.h"

#include <assert.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

void bytebuf_init(bytebuf* buf, int initial_cap)
{
    buf->buf = malloc(initial_cap);
    buf->cap = initial_cap;
    buf->len = 0;
}

void bytebuf_destroy(bytebuf* buf)
{
    free(buf->buf);
}

void bytebuf_reserve(bytebuf* buf, int amt)
{
    if (buf->cap - buf->len < amt)
    {
        int new_cap = buf->cap * 2;
        while (new_cap < buf->len + amt)
            new_cap *= 2;

        buf->buf = realloc(buf->buf, new_cap);
        buf->cap = new_cap;
    }
}

void bytebuf_push_int(bytebuf* buf, int i)
{
    bytebuf_reserve(buf, sizeof(int));
    *(int*)(buf->buf + buf->len) = i;
    buf->len += sizeof(int);
}

void bytebuf_drop(bytebuf* buf, int amt)
{
    assert(amt <= buf->len);
    memmove(buf->buf, buf->buf + amt, buf->len - amt);
    buf->len -= amt;
}

void msg_buf_init(msg_buf* buf)
{
    // buffers big enough to hold 50 IRC messages
    bytebuf_init(&buf->msg_buf,  50 * 512);
    bytebuf_init(&buf->msg_idxs, 50 * sizeof(int));
}

void msg_buf_destroy(msg_buf* buf)
{
    bytebuf_destroy(&buf->msg_buf);
    bytebuf_destroy(&buf->msg_idxs);
}

void msg_buf_append_filedes(msg_buf* buf, int filedes)
{
    bytebuf_reserve(&buf->msg_buf, 4096);
    int read_ret = read(filedes, buf->msg_buf.buf + buf->msg_buf.len, 4096);
    assert(read_ret >= 0);
    buf->msg_buf.len += read_ret;

    // Update msg_idxs
    int last_msg_idx = 0;
    if (buf->msg_idxs.len != 0)
        last_msg_idx = ((int*)(buf->msg_idxs.buf))[(buf->msg_idxs.len / sizeof(int)) - 1];

    while (last_msg_idx < buf->msg_buf.len - 1)
    {
        if (buf->msg_buf.buf[last_msg_idx] == '\r' && buf->msg_buf.buf[last_msg_idx + 1] == '\n')
        {
            bytebuf_push_int(&buf->msg_idxs, last_msg_idx + 2);
            last_msg_idx += 2;
        }
        else
            last_msg_idx += 1;
    }
}

irc_msg* msg_buf_extract_msgs(msg_buf* buf)
{
    irc_msg*  head = NULL;
    irc_msg** tail = &head;

    int last_idx = 0;
    for (int i = 0; i < buf->msg_idxs.len / (int)sizeof(int); ++i)
    {
        int idx = ((int*)buf->msg_idxs.buf)[i];

        int msg_len = idx - last_idx - 2;
        uint8_t* str = malloc(msg_len + 1);
        memcpy(str, buf->msg_buf.buf + last_idx, msg_len);
        str[msg_len] = 0;

        *tail = malloc(sizeof(irc_msg));
        (*tail)->contents = str;
        (*tail)->len = msg_len;
        (*tail)->next = NULL;
        tail = &((*tail)->next);

        last_idx = idx;
    }

    // clear consumed parts of the buffers
    bytebuf_drop(&buf->msg_buf,  last_idx);
    bytebuf_drop(&buf->msg_idxs, buf->msg_idxs.len);

    return head;
}

void irc_msg_free(irc_msg* msgs)
{
    irc_msg* next = NULL;
    do
    {
        next = msgs->next;
        free(msgs->contents);
        free(msgs);
        msgs = next;
    } while (next != NULL);
}
