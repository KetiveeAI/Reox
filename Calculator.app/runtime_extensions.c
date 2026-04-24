#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdbool.h>

// Simple string type simulation (Reox strings are char*)
typedef char* string;

// File Append
void append_file(string path, string content) {
    FILE* f = fopen(path, "a");
    if (f) {
        fprintf(f, "%s", content);
        fclose(f);
    }
}

// Global State
static char current_display[256] = "0";
static double accumulator = 0.0;
static char pending_op[16] = "";
static bool new_entry = true;
static bool settings_visible = false;

// Accessors
string get_display() {
    return strdup(current_display); // Return copy as Reox likely manages memory or expects valid ptr
}

void set_display(string s) {
    strncpy(current_display, s, 255);
}

double get_accumulator() { return accumulator; }
void set_accumulator(double v) { accumulator = v; }

string get_pending_op() { return strdup(pending_op); }
void set_pending_op(string s) { strncpy(pending_op, s, 15); }

bool get_new_entry() { return new_entry; }
void set_new_entry(bool v) { new_entry = v; }

bool get_settings_visible() { return settings_visible; }
void set_settings_visible(bool v) { settings_visible = v; }

// UI Handles (Mocking View struct which is just an int handle)
static int display_label_handle = -1;
int get_display_label_handle() { return display_label_handle; }
void set_display_label_handle(int h) { display_label_handle = h; }

static int root_view_handle = -1;
int get_root_view_handle() { return root_view_handle; }
void set_root_view_handle(int h) { root_view_handle = h; }

// Helpers
void int_to_string_impl(int i, char* buf) { sprintf(buf, "%d", i); }
string int_to_string(int i) {
    char buf[32];
    sprintf(buf, "%d", i);
    return strdup(buf);
}
string float_to_string(double f) {
    char buf[64];
    sprintf(buf, "%.6g", f); // Clean formatting
    return strdup(buf);
}
string string_concat(string a, string b) {
    char* res = malloc(strlen(a) + strlen(b) + 1);
    strcpy(res, a);
    strcat(res, b);
    return res;
}
bool string_eq(string a, string b) {
    return strcmp(a, b) == 0;
}

// Helper for Reox (struct layout must match Reox generated C: int64_t)
typedef struct { int64_t r; int64_t g; int64_t b; int64_t a; } ReoxColor;

ReoxColor get_button_color_wrapper(bool is_op, bool is_action, int state) {
    // state: 0 = base, 1 = hover
    ReoxColor c;
    if (state == 0) { // Base
        if (is_op) { c = (ReoxColor){100, 100, 240, 255}; } // Secondary (sort of)
        else if (is_action) { c = (ReoxColor){50, 150, 255, 255}; } // Primary
        else { c = (ReoxColor){40, 40, 40, 255}; } // Surface
    } else { // Hover
        if (is_op) { c = (ReoxColor){120, 120, 255, 255}; }
        else if (is_action) { c = (ReoxColor){70, 170, 255, 255}; }
        else { c = (ReoxColor){60, 60, 65, 255}; }
    }
    return c;
}

double string_to_float(string s) {
    return atof(s);
}
