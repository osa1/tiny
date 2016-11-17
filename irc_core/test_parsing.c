#include "src/message.h"

#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/stat.h>

static const char* messages_file = "messages.txt";

int main()
{
    struct stat st;
    stat(messages_file, &st);
    int size = st.st_size;

    int msg_file = open(messages_file, O_RDONLY);
    char* data = mmap(0, size, PROT_READ, MAP_PRIVATE, msg_file, 0);

    if (data == MAP_FAILED)
    {
        printf("%s\n", strerror(errno));
        exit(1);
    }

    // printf("%.*s\n", size, data);

    int line_begins = 0;
    for (int i = 0; i < size; ++i)
    {
        if (data[i] == '\n')
        {
            int size = i - line_begins + 2;
            char* str = malloc(size);
            memcpy(str, data + line_begins, size);
            str[size - 2] = '\r';
            str[size - 1] = '\n';
            printf("parsing: %.*s\n", size - 2, str);
            
            message* msg = message_parse(str, size);
            free(str);
            if (!msg)
                printf("parse failed.\n");
            else
            {
                message_print(msg);
                message_free(msg);
            }
            printf("\n");


            line_begins = i + 1;
        }
    }


    return 0;
}
