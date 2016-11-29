#ifndef __MT_QUEUE_H
#define __MT_QUEUE_H

/**
 * A queue that allows 'push' and 'pop' to be called in different threads.
 */

typedef struct mt_queue_ mt_queue;

mt_queue* mt_queue_new(int initial_cap);

void mt_queue_free(mt_queue*);

void mt_queue_push(mt_queue*, void*);

/**
 * Blocks until queue has a value. See 'mt_queue_try_pop' for non-blocking
 * version.
 */
void* mt_queue_pop(mt_queue*);

/** Returns an eventfd that'll be ready for reading when queue is not empty. */
int mt_queue_get_eventfd(mt_queue*);

#endif
