struct bytebuffer {
    char *buf;
    int len;
    int cap;
};

static void bytebuffer_reserve(struct bytebuffer *b, int cap) {
    if (b->cap >= cap) {
        return;
    }

    // prefer doubling capacity
    if (b->cap * 2 >= cap) {
        cap = b->cap * 2;
    }

    char *newbuf = realloc(b->buf, cap);
    b->buf = newbuf;
    b->cap = cap;
}

static void bytebuffer_init(struct bytebuffer *b, int cap) {
    b->cap = 0;
    b->len = 0;
    b->buf = 0;

    if (cap > 0) {
        b->cap = cap;
        b->buf = malloc(cap); // just assume malloc works always
    }
}

static void bytebuffer_free(struct bytebuffer *b) {
    if (b->buf)
        free(b->buf);
}

static void bytebuffer_clear(struct bytebuffer *b) {
    b->len = 0;
}

static void bytebuffer_append(struct bytebuffer *b, const char *data, int len) {
    bytebuffer_reserve(b, b->len + len);
    memcpy(b->buf + b->len, data, len);
    b->len += len;
}

static void bytebuffer_append_utf8_char(struct bytebuffer *b, uint32_t ch) {
    bytebuffer_reserve(b, b->len + 4);
    if (ch <= 0x7F)
    {
        b->buf[b->len] = ch;
        b->len += 1;
    }
    else if (ch >= 0xC080 && ch <= 0xDFBF)
    {
        b->buf[b->len + 0] = ch >> 8;
        b->buf[b->len + 1] = ch;
        b->len += 2;
    }
    else if (ch >= 0xE08080 && ch <= 0xEFBFBF)
    {
        b->buf[b->len + 0] = ch >> 16;
        b->buf[b->len + 1] = ch >> 8;
        b->buf[b->len + 2] = ch;
        b->len += 3;
    }
    else if (ch >= 0xF0808080 && ch <= 0xF7BFBFBF)
    {
        b->buf[b->len + 0] = ch >> 24;
        b->buf[b->len + 1] = ch >> 16;
        b->buf[b->len + 2] = ch >> 8;
        b->buf[b->len + 3] = ch;
        b->len += 4;
    }
    else
    {
        fprintf(stderr, "invalid utf8 character: %d\n", ch);
    }
}

static void bytebuffer_puts(struct bytebuffer *b, const char *str) {
    bytebuffer_append(b, str, strlen(str));
}

static void bytebuffer_flush(struct bytebuffer *b, int fd) {
    write(fd, b->buf, b->len);
    bytebuffer_clear(b);
}
