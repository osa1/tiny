enum {
    T_ENTER_CA,
    T_EXIT_CA,
    T_SHOW_CURSOR,
    T_HIDE_CURSOR,
    T_CLEAR_SCREEN,
    T_SGR0,
    T_UNDERLINE,
    T_BOLD,
    T_BLINK,
    T_REVERSE,
    T_ENTER_KEYPAD,
    T_EXIT_KEYPAD,
    T_ENABLE_FOCUS_EVENTS,
    T_DISABLE_FOCUS_EVENTS,
    T_ENTER_MOUSE,
    T_EXIT_MOUSE,
    T_FUNCS_NUM,
};

#define ENTER_MOUSE_SEQ "\x1b[?1000h\x1b[?1002h\x1b[?1015h\x1b[?1006h"
#define EXIT_MOUSE_SEQ "\x1b[?1006l\x1b[?1015l\x1b[?1002l\x1b[?1000l"
#define ENABLE_FOCUS_SEQ "\033[?1004h"
#define DISABLE_FOCUS_SEQ "\033[?1004l"

#define EUNSUPPORTED_TERM -1

static const char **funcs;

//----------------------------------------------------------------------
// terminfo
//----------------------------------------------------------------------

static char *read_file(const char *file) {
    FILE *f = fopen(file, "rb");
    if (!f)
        return 0;

    struct stat st;
    if (fstat(fileno(f), &st) != 0) {
        fclose(f);
        return 0;
    }

    char *data = malloc(st.st_size);
    if (!data) {
        fclose(f);
        return 0;
    }

    if (fread(data, 1, st.st_size, f) != (size_t)st.st_size) {
        fclose(f);
        free(data);
        return 0;
    }

    fclose(f);
    return data;
}

static char *terminfo_try_path(const char *path, const char *term) {
    char tmp[4096];
    snprintf(tmp, sizeof(tmp), "%s/%c/%s", path, term[0], term);
    tmp[sizeof(tmp)-1] = '\0';
    char *data = read_file(tmp);
    if (data) {
        return data;
    }

    // fallback to darwin specific dirs structure
    snprintf(tmp, sizeof(tmp), "%s/%x/%s", path, term[0], term);
    tmp[sizeof(tmp)-1] = '\0';
    return read_file(tmp);
}

static char *load_terminfo(void) {
    char tmp[4096];
    const char *term = getenv("TERM");
    if (!term) {
        return 0;
    }

    // if TERMINFO is set, no other directory should be searched
    const char *terminfo = getenv("TERMINFO");
    if (terminfo) {
        return terminfo_try_path(terminfo, term);
    }

    // next, consider ~/.terminfo
    const char *home = getenv("HOME");
    if (home) {
        snprintf(tmp, sizeof(tmp), "%s/.terminfo", home);
        tmp[sizeof(tmp)-1] = '\0';
        char *data = terminfo_try_path(tmp, term);
        if (data)
            return data;
    }

    // next, TERMINFO_DIRS
    const char *dirs = getenv("TERMINFO_DIRS");
    if (dirs) {
        snprintf(tmp, sizeof(tmp), "%s", dirs);
        tmp[sizeof(tmp)-1] = '\0';
        char *dir = strtok(tmp, ":");
        while (dir) {
            const char *cdir = dir;
            if (strcmp(cdir, "") == 0) {
                cdir = "/usr/share/terminfo";
            }
            char *data = terminfo_try_path(cdir, term);
            if (data)
                return data;
            dir = strtok(0, ":");
        }
    }

    // fallback to /usr/share/terminfo
    return terminfo_try_path("/usr/share/terminfo", term);
}

#define TI_HEADER_LENGTH 12

static const char *terminfo_copy_string(char *data, int str, int table) {
    const int16_t off = *(int16_t*)(data + str);
    const char *src = data + table + off;
    int len = strlen(src);
    char *dst = malloc(len+1);
    strcpy(dst, src);
    return dst;
}

static const int16_t ti_funcs[] = {
    28, 40, 16, 13, 5, 39, 36, 27, 26, 34, 89, 88,
};

static int init_term(void) {
    int i;
    char *data = load_terminfo();
    if (!data) {
        return EUNSUPPORTED_TERM;
    }

    int16_t *header = (int16_t*)data;
    if ((header[1] + header[2]) % 2) {
        // old quirk to align everything on word boundaries
        header[2] += 1;
    }

    const int str_offset = TI_HEADER_LENGTH +
        header[1] + header[2] +    2 * header[3];
    const int table_offset = str_offset + 2 * header[4];

    funcs = malloc(sizeof(const char*) * T_FUNCS_NUM);
    // the last two entries are reserved for mouse. because the table offset is
    // not there, the two entries have to fill in manually
    for (i = 0; i < T_FUNCS_NUM-4; i++) {
        funcs[i] = terminfo_copy_string(data,
            str_offset + 2 * ti_funcs[i], table_offset);
    }

    funcs[T_ENABLE_FOCUS_EVENTS] = ENABLE_FOCUS_SEQ;
    funcs[T_DISABLE_FOCUS_EVENTS] = DISABLE_FOCUS_SEQ;
    funcs[T_ENTER_MOUSE] = ENTER_MOUSE_SEQ;
    funcs[T_EXIT_MOUSE] = EXIT_MOUSE_SEQ;

    free(data);
    return 0;
}

static void shutdown_term(void) {
    int i;
    // the last two entries are reserved for mouse. because the table offset
    // is not there, the two entries have to fill in manually and do not
    // need to be freed.
    for (i = 0; i < T_FUNCS_NUM-4; i++) {
        free((void*)funcs[i]);
    }
    free(funcs);
}
