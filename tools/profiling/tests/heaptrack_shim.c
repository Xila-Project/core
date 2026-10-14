// Test stand-in for heaptrack's public custom allocator API. No heap allocation.
#include <fcntl.h>
#include <stdlib.h>
#include <stdio.h>
#include <unistd.h>

void heaptrack_malloc(void *pointer, size_t size) {
    int fd = open(getenv("XILA_TEST_HEAP_EVENTS"), O_WRONLY | O_CREAT | O_APPEND, 0600);
    dprintf(fd, "alloc %p %zu\n", pointer, size);
    close(fd);
}
void heaptrack_free(void *pointer) {
    int fd = open(getenv("XILA_TEST_HEAP_EVENTS"), O_WRONLY | O_CREAT | O_APPEND, 0600);
    dprintf(fd, "free %p\n", pointer);
    close(fd);
}
