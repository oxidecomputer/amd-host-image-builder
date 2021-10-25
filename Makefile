
.PHONY: all
all:
	cargo run -- -g Milan -o Milan.img
	cargo run -- -g Rome -o Rome.img
