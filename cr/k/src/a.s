.text
# w - eight chars at a time, stay inside the slice
.globl bt_w
.p2align 4
bt_w:
    xor eax, eax
    pxor xmm1, xmm1
.Lw_loop:
    cmp rdx, 8
    jb .Lw_tail
    movdqu xmm0, [rcx + rax*2]
    pcmpeqw xmm0, xmm1
    pmovmskb r8d, xmm0
    test r8d, r8d
    jnz .Lw_hit
    add rax, 8
    sub rdx, 8
    jmp .Lw_loop
.Lw_hit:
    bsf r8d, r8d
    shr r8d, 1
    add rax, r8
    ret
.Lw_tail:
    test rdx, rdx
    jz .Lw_ret
    cmp word ptr [rcx + rax*2], 0
    je .Lw_ret
    inc rax
    dec rdx
    jmp .Lw_tail
.Lw_ret:
    ret

# p - lower ascii and fix slashes in one pass
.globl bt_p
.p2align 4
bt_p:
    xor r8d, r8d
.Lp_loop:
    cmp rdx, 16
    jb .Lp_tail
    movdqu xmm0, [rcx]
    pmovmskb eax, xmm0
    or r8d, eax
    movdqa xmm1, xmm0
    pcmpgtb xmm1, [rip + .Lb40]
    movdqa xmm2, [rip + .Lb5b]
    pcmpgtb xmm2, xmm0
    pand xmm1, xmm2
    pand xmm1, [rip + .Lb20]
    por xmm0, xmm1
    movdqa xmm1, xmm0
    pcmpeqb xmm1, [rip + .Lb2f]
    pand xmm1, [rip + .Lb73]
    pxor xmm0, xmm1
    movdqu [rcx], xmm0
    add rcx, 16
    sub rdx, 16
    jmp .Lp_loop
.Lp_tail:
    test rdx, rdx
    jz .Lp_ret
    movzx eax, byte ptr [rcx]
    mov r9d, eax
    and r9d, 0x80
    or r8d, r9d
    lea r9d, [rax - 65]
    cmp r9d, 25
    ja .Lp_sl
    or eax, 0x20
.Lp_sl:
    cmp eax, 47
    jne .Lp_put
    mov eax, 92
.Lp_put:
    mov [rcx], al
    inc rcx
    dec rdx
    jmp .Lp_tail
.Lp_ret:
    xor eax, eax
    test r8d, r8d
    setne al
    ret

# i - read the pid right from pdh utf16
.globl bt_i
.p2align 4
bt_i:
    cmp rdx, 6
    jb .Li_bad
    mov rax, 0x005f006400690070
    cmp [rcx], rax
    jne .Li_bad
    mov r9d, 4
    xor r8d, r8d
    mov r11d, 0xffffffff
.Li_loop:
    cmp r9, rdx
    jae .Li_bad
    movzx eax, word ptr [rcx + r9*2]
    cmp eax, 95
    je .Li_end
    sub eax, 48
    cmp eax, 9
    ja .Li_bad
    imul r8, r8, 10
    add r8, rax
    cmp r8, r11
    ja .Li_bad
    inc r9
    cmp r9, 15
    jb .Li_loop
    jmp .Li_bad
.Li_end:
    cmp r9, 4
    je .Li_bad
    inc r9
    shl r9, 32
    mov eax, r8d
    or rax, r9
    ret
.Li_bad:
    mov rax, -1
    ret

# e - fold ascii names; rust handles the unicode bits
.globl bt_e
.p2align 4
bt_e:
    pxor xmm5, xmm5
.Le_loop:
    cmp r8, 8
    jb .Le_tail
    movdqu xmm0, [rcx]
    movdqu xmm1, [rdx]
    movdqa xmm2, xmm0
    por xmm2, xmm1
    pand xmm2, [rip + .Lwhi]
    pcmpeqw xmm2, xmm5
    pmovmskb eax, xmm2
    cmp eax, 65535
    jne .Le_uni
    movdqa xmm2, xmm0
    pcmpgtw xmm2, [rip + .Lw40]
    movdqa xmm3, [rip + .Lw5b]
    pcmpgtw xmm3, xmm0
    pand xmm2, xmm3
    pand xmm2, [rip + .Lw20]
    por xmm0, xmm2
    movdqa xmm2, xmm1
    pcmpgtw xmm2, [rip + .Lw40]
    movdqa xmm3, [rip + .Lw5b]
    pcmpgtw xmm3, xmm1
    pand xmm2, xmm3
    pand xmm2, [rip + .Lw20]
    por xmm1, xmm2
    pcmpeqw xmm0, xmm1
    pmovmskb eax, xmm0
    cmp eax, 65535
    jne .Le_no
    add rcx, 16
    add rdx, 16
    sub r8, 8
    jmp .Le_loop
.Le_tail:
    test r8, r8
    jz .Le_yes
    movzx r9d, word ptr [rcx]
    movzx r10d, word ptr [rdx]
    mov eax, r9d
    or eax, r10d
    cmp eax, 127
    ja .Le_uni
    lea r11d, [r9 - 65]
    cmp r11d, 25
    ja .Le_b
    or r9d, 32
.Le_b:
    lea r11d, [r10 - 65]
    cmp r11d, 25
    ja .Le_cmp
    or r10d, 32
.Le_cmp:
    cmp r9d, r10d
    jne .Le_no
    add rcx, 2
    add rdx, 2
    dec r8
    jmp .Le_tail
.Le_yes:
    mov eax, 1
    ret
.Le_uni:
    mov eax, 2
    ret
.Le_no:
    xor eax, eax
    ret

.section .rdata,"dr"
.p2align 4
.Lb40: .fill 16, 1, 0x40
.Lb5b: .fill 16, 1, 0x5b
.Lb20: .fill 16, 1, 0x20
.Lb2f: .fill 16, 1, 0x2f
.Lb73: .fill 16, 1, 0x73
.Lw40: .fill 8, 2, 0x0040
.Lw5b: .fill 8, 2, 0x005b
.Lw20: .fill 8, 2, 0x0020
.Lwhi: .fill 8, 2, 0xff80