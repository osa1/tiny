#include <pthread.h>
#include <stdint.h>
#include <stdio.h>
#include <unistd.h>
#include <stdlib.h>
#include <string.h>

#include "src/deque.h"
#include "src/mt_deque.h"
#include "src/msg_buf.h"

#define ANSI_COLOR_RED     "\x1b[31m"
#define ANSI_COLOR_GREEN   "\x1b[32m"
#define ANSI_COLOR_RESET   "\x1b[0m"

#define assert(ans, str) \
{ \
    printf("\t%s:\t", str); \
    if ((ans) == 1) \
        printf(ANSI_COLOR_GREEN "OK\n" ANSI_COLOR_RESET); \
    else \
    { \
        printf(ANSI_COLOR_RED "ERR\n" ANSI_COLOR_RESET); \
        return 1; \
    } \
}

int test_single_thread();
int st1();
int st2();

int test_multi_thread();
int mt1();

int test_msg_buf();

int main()
{
    int ret = 0;

    ret |= test_single_thread();
    ret |= test_multi_thread();
    ret |= test_msg_buf();

    return ret;
}

int test_single_thread()
{
    int ret = 0;

    printf("=== Running single threaded tests ===\n");
    ret |= st1();
    ret |= st2();

    return ret;
}

int st1()
{
    deque* d = deque_new(1);

    deque_push_front(d, (void*)1);
    deque_push_front(d, (void*)2);
    deque_push_front(d, (void*)3);
    assert((uint64_t)deque_pop_back(d) == 1, "pop_back()");
    assert((uint64_t)deque_pop_back(d) == 2, "pop_back()");
    assert((uint64_t)deque_pop_back(d) == 3, "pop_back()");

    deque_push_back(d, (void*)3);
    deque_push_back(d, (void*)2);
    deque_push_back(d, (void*)1);
    assert((uint64_t)deque_pop_front(d) == 3, "pop_front()");
    assert((uint64_t)deque_pop_front(d) == 2, "pop_front()");
    assert((uint64_t)deque_pop_front(d) == 1, "pop_front()");
    assert(deque_pop_front(d) == NULL,        "pop_front()");

    deque_free(d);

    return 0;
}

int st2()
{
    int ret = 0;

    mt_deque* md = mt_deque_new(1);

    for (uint64_t i = 0; i < 100; ++i)
        if (rand() % 2)
            mt_deque_push_front(md, (void*)i);
        else
            mt_deque_push_back(md, (void*)i);

    uint64_t buf[100] = { 0 };
    for (int i = 0; i < 100; ++i)
        if (rand() % 2)
            buf[(uint64_t)mt_deque_pop_front(md)] = 1;
        else
            buf[(uint64_t)mt_deque_pop_back(md)] = 1;

    for (int i = 0; i < 100; ++i)
        ret |= buf[i] == 0;

    mt_deque_free(md);

    assert(ret == 0, "st2");

    return ret;
}

int test_multi_thread()
{
    int ret = 0;

    printf("=== Running multi threaded tests ====\n");
    for (int i = 0; i < 100; ++i)
    {
        printf("Iteration: %d\n", i);
        fflush(stdout);
        ret |= mt1();
        if (ret) break;
        // sleep(1);
    }

    return ret;
}

struct writer_state
{
    mt_deque* md;
};

// Writer thread
void* wr_fn(struct writer_state* st)
{
    for (int64_t i = 0; i < 100; ++i)
        if (rand() % 2)
            mt_deque_push_front(st->md, (void*)i);
        else
            mt_deque_push_back(st->md, (void*)i);

    return NULL;
}

struct reader_state
{
    mt_deque* md;
    int nums[100];
};

// Reader thread
void* rd_fn(struct reader_state* st)
{
    int64_t nums[100];
    memset(nums, 0, sizeof(int64_t) * 100);

    for (int i = 0; i < 100; ++i)
    {
        if (rand() % 2)
            nums[(int64_t)mt_deque_pop_front(st->md)] = 1;
        else
            nums[(int64_t)mt_deque_pop_back(st->md)] = 1;
    }

    for (int i = 0; i < 100; ++i)
        if (nums[i] == 0)
            return (void*)1;

    return NULL;
}

int mt1()
{
    // Idea: Thread 1 pushes numbers from 1 to 100 to a 'mt_deque', thread 2
    // pops and expects to see numbers from 1 to 100. Front/back is chosen
    // randomly.

    mt_deque* md = mt_deque_new(1);

    struct writer_state wr_state = { .md = md };
    struct reader_state rd_state = { .md = md, .nums = { 0 } };

    pthread_t wr_thr, rd_thr;
    pthread_create(&wr_thr, NULL, (void*(*)(void*))wr_fn, &wr_state);
    pthread_create(&rd_thr, NULL, (void*(*)(void*))rd_fn, &rd_state);

    void* thr_ret;
    assert(pthread_join(wr_thr, NULL) == 0, "join writer thread");
    assert(pthread_join(rd_thr, &thr_ret) == 0, "join reader thread");

    mt_deque_free(md);

    printf("thr_ret: %p\n", thr_ret);
    assert(thr_ret == NULL, "multi-threaded push/pop");

    return 0;
}

int test_msg_buf()
{
    msg_buf buf;
    msg_buf_init(&buf);

    int pipefd[2]; // { read end, write end }
    pipe(pipefd);

    write(pipefd[1], "msg1\r\nmsg2\r\n", 12);
    msg_buf_append_fd(&buf, pipefd[0]);

    irc_msg* msgs = msg_buf_extract_msgs(&buf);
    assert(msgs != NULL, "msg_buf_extract_msgs() returned something");

    char err_msg[100];

    sprintf(err_msg, "checking first message: \"%s\"", msgs->contents);
    assert(strncmp((char*)msgs->contents, "msg1", 4) == 0, err_msg);

    assert(msgs->next != NULL && msgs->next->next == NULL,
           "msg_buf_extract_msgs() returned two msgs");

    sprintf(err_msg, "checking second message: \"%s\"", msgs->next->contents);
    assert(strncmp((char*)msgs->next->contents, "msg2", 4) == 0, err_msg);

    assert(buf.msg_buf.len == 0, "message buffer is empty");
    assert(buf.msg_idxs.len == 0, "index buffer is empty");

    irc_msg_free(msgs);
    msg_buf_destroy(&buf);
    return 0;
}
