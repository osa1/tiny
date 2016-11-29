#ifndef __MSG_BUF_H
#define __MSG_BUF_H

#include <stdint.h>

typedef struct bytebuf_
{
    uint8_t*    buf;
    int         cap;
    int         len;
} bytebuf;

void bytebuf_init(bytebuf*, int initial_cap);
void bytebuf_destroy(bytebuf*);
void bytebuf_reserve(bytebuf*, int amt);
void bytebuf_push_int(bytebuf*, int);
void bytebuf_push_bytes(bytebuf*, uint8_t* str, int amt);
void bytebuf_drop(bytebuf*, int amt);
void bytebuf_write_fd(bytebuf*, int fd);

/// A buffer for 0x0D 0x0A ("\r\n") terminated messages.
typedef struct msg_buf_
{
    /// Messages are collected here.
    bytebuf msg_buf;
    /// Keep indices to msg beginnings (this in an 'int' buffer).
    /// First message always starts at index 0 and we don't have an index in
    /// this buffer for that.
    bytebuf msg_idxs;
} msg_buf;

void msg_buf_init(msg_buf*);
void msg_buf_destroy(msg_buf*);
int msg_buf_append_fd(msg_buf*, int fd);

typedef struct irc_msg_
{
    /**
     * A complete IRC message (includes \r\n suffix). Null-terminated to make
     * debugging easier.
     */
    uint8_t*            contents;
    int                 len;
    struct irc_msg_*    next;
} irc_msg;

irc_msg* msg_buf_extract_msgs(msg_buf*);
void irc_msg_free(irc_msg*);

#endif
