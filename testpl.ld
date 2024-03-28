/* Trivial test payload. */

ENTRY(reset);

SECTIONS {
	__sloader = .;
	.text: . : {
		. = 65536 - 16;
		*(.text*)
		. = 65536;
	}
	.rodata: . : {
		*(.rodata*)
		. = ALIGN(4096);
	}
	.data: . : {
		*(.data*)
		. = ALIGN(4096);
	}
	.bss: . : {
		*(.bss* COMMON)
		. = ALIGN(4096);
	}

	__eloader = .;

	/DISCARD/ : {
		*(.got* .comment* .note* .eh_frame*)
	}

	_BL_SPACE = __eloader - __sloader;
}
