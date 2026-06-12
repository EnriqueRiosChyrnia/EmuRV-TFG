/* Fixture RP11: imprime "42\n" con la convención RARS y sale con 93. */
static void print_int(int v) {
    register int a0 __asm__("a0") = v;
    register int a7 __asm__("a7") = 1;
    __asm__ volatile("ecall" : : "r"(a0), "r"(a7));
}
static void print_chr(int c) {
    register int a0 __asm__("a0") = c;
    register int a7 __asm__("a7") = 11;
    __asm__ volatile("ecall" : : "r"(a0), "r"(a7));
}
void _start(void) {
    print_int(42);
    print_chr(10);
    register int a0 __asm__("a0") = 0;
    register int a7 __asm__("a7") = 93;
    __asm__ volatile("ecall" : : "r"(a0), "r"(a7));
    for (;;) {}
}
