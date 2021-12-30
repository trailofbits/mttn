int add(int x, int y) {
  return x + y;
}

int sub(int x, int y) {
  return x - y;
}

int mul(int x, int y) {
  return x * y;
}

int div(int x, int y) {
  return x / y;
}

typedef int (*handler)(int, int);

handler table[4] = { add, sub, mul, div };

int dispatch(int op, int x, int y) {
  return table[op](x, y);
}

int main(int argc, char **argv) {
  dispatch(0, 2, 3);
  dispatch(1, 2, 3);
  dispatch(2, 2, 3);
  dispatch(3, 2, 3);

  // HACK: Call exit(2) explicitly so that we don't fall off the end of the world.
  __asm__(
    "mov $1, %eax\n\t"
    "int $0x80\n\t"
  );

  return 0; // unreachable
}
