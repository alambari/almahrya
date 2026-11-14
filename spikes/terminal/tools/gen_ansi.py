"""Write about 50 MiB of SGR-coloured text lines to the path given (spike test data)."""
import random
import sys

random.seed(7)
WORDS = ["alpha", "beta", "gamma", "delta", "omega", "src/main.rs", "Cargo.toml", "error", "warning", "ok"]
COLOURS = [31, 32, 33, 34, 35, 36, "1;37", "38;5;208", "38;2;111;124;242"]
TARGET = 52428800
with open(sys.argv[1], "w") as out:
    n = 0
    while n < TARGET:
        parts = ["\x1b[%sm%s\x1b[0m" % (random.choice(COLOURS), random.choice(WORDS)) for _ in range(8)]
        line = " ".join(parts) + "\n"
        out.write(line)
        n += len(line)
