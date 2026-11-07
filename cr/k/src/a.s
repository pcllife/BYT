# SAT
.text
.globl bt_d
.p2align 4
bt_d:
    mov rax, rcx
    sub rax, rdx
    jnc 2f
    xor eax, eax
2:
    ret
.globl bt_a
.p2align 4
bt_a:
    mov rax, rcx
    add rax, rdx
    jnc 3f
    mov rax, -1
3:
    ret
