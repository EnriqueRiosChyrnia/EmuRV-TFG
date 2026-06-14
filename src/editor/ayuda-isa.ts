// Diccionario de ayuda por instrucción (RP14-F3): sintaxis + descripción
// en español para el tooltip de hover. El material CONDENSA la Ayuda
// integrada (src/paneles/ayuda.ts), las lecciones (lecciones/) y
// docs/ENSAMBLADOR.md — NO se inventa semántica. Un mnemónico sin entrada
// acá NO muestra tooltip (nunca uno vacío o erróneo).
//
// Cobertura 100% de isa.ts garantizada por test
// (src/editor/ayuda-isa.test.ts): si se agrega una instrucción a isa.ts
// sin entrada acá, el test falla.

export interface AyudaInstr {
  sintaxis: string;
  descripcion: string;
}

/// Clave = mnemónico (forma base; los sufijos .aq/.rl/.aqrl de las
/// atómicas se resuelven a la base en `descripcionInstr`).
export const AYUDA_ISA: Record<string, AyudaInstr> = {
  // ---- RV32I ----
  lui: { sintaxis: "lui rd, imm20", descripcion: "Carga un inmediato de 20 bits en la parte alta de rd; los 12 bits bajos quedan en 0." },
  auipc: { sintaxis: "auipc rd, imm20", descripcion: "rd ← pc + (imm20 << 12). Base para direcciones relativas al pc." },
  jal: { sintaxis: "jal rd, etiqueta", descripcion: "Salta a la etiqueta y guarda la dirección de retorno (pc+4) en rd." },
  jalr: { sintaxis: "jalr rd, imm(rs1)", descripcion: "Salto indirecto a rs1+imm, guardando el retorno (pc+4) en rd." },
  beq: { sintaxis: "beq rs1, rs2, etiqueta", descripcion: "Salta si rs1 == rs2." },
  bne: { sintaxis: "bne rs1, rs2, etiqueta", descripcion: "Salta si rs1 != rs2." },
  blt: { sintaxis: "blt rs1, rs2, etiqueta", descripcion: "Salta si rs1 < rs2 (con signo)." },
  bge: { sintaxis: "bge rs1, rs2, etiqueta", descripcion: "Salta si rs1 >= rs2 (con signo)." },
  bltu: { sintaxis: "bltu rs1, rs2, etiqueta", descripcion: "Salta si rs1 < rs2 (sin signo)." },
  bgeu: { sintaxis: "bgeu rs1, rs2, etiqueta", descripcion: "Salta si rs1 >= rs2 (sin signo)." },
  lb: { sintaxis: "lb rd, imm(rs1)", descripcion: "Carga un byte con signo desde memoria a rd." },
  lh: { sintaxis: "lh rd, imm(rs1)", descripcion: "Carga media palabra (2 bytes) con signo a rd." },
  lw: { sintaxis: "lw rd, imm(rs1)", descripcion: "Carga una palabra (4 bytes) desde memoria a rd." },
  lbu: { sintaxis: "lbu rd, imm(rs1)", descripcion: "Carga un byte SIN signo (extiende con ceros) a rd." },
  lhu: { sintaxis: "lhu rd, imm(rs1)", descripcion: "Carga media palabra SIN signo a rd." },
  sb: { sintaxis: "sb rs2, imm(rs1)", descripcion: "Guarda el byte bajo de rs2 en memoria." },
  sh: { sintaxis: "sh rs2, imm(rs1)", descripcion: "Guarda los 2 bytes bajos de rs2 en memoria." },
  sw: { sintaxis: "sw rs2, imm(rs1)", descripcion: "Guarda la palabra rs2 (4 bytes) en memoria." },
  addi: { sintaxis: "addi rd, rs1, imm", descripcion: "rd ← rs1 + inmediato (12 bits con signo)." },
  slti: { sintaxis: "slti rd, rs1, imm", descripcion: "rd ← 1 si rs1 < imm (con signo), si no 0." },
  sltiu: { sintaxis: "sltiu rd, rs1, imm", descripcion: "rd ← 1 si rs1 < imm (sin signo), si no 0." },
  xori: { sintaxis: "xori rd, rs1, imm", descripcion: "rd ← rs1 XOR inmediato." },
  ori: { sintaxis: "ori rd, rs1, imm", descripcion: "rd ← rs1 OR inmediato." },
  andi: { sintaxis: "andi rd, rs1, imm", descripcion: "rd ← rs1 AND inmediato." },
  slli: { sintaxis: "slli rd, rs1, shamt", descripcion: "Corrimiento lógico a la izquierda por una cantidad fija." },
  srli: { sintaxis: "srli rd, rs1, shamt", descripcion: "Corrimiento lógico a la derecha (entra 0)." },
  srai: { sintaxis: "srai rd, rs1, shamt", descripcion: "Corrimiento aritmético a la derecha (copia el signo)." },
  add: { sintaxis: "add rd, rs1, rs2", descripcion: "rd ← rs1 + rs2." },
  sub: { sintaxis: "sub rd, rs1, rs2", descripcion: "rd ← rs1 - rs2." },
  sll: { sintaxis: "sll rd, rs1, rs2", descripcion: "Corrimiento lógico a la izquierda por rs2." },
  slt: { sintaxis: "slt rd, rs1, rs2", descripcion: "rd ← 1 si rs1 < rs2 (con signo)." },
  sltu: { sintaxis: "sltu rd, rs1, rs2", descripcion: "rd ← 1 si rs1 < rs2 (sin signo)." },
  xor: { sintaxis: "xor rd, rs1, rs2", descripcion: "rd ← rs1 XOR rs2." },
  srl: { sintaxis: "srl rd, rs1, rs2", descripcion: "Corrimiento lógico a la derecha por rs2." },
  sra: { sintaxis: "sra rd, rs1, rs2", descripcion: "Corrimiento aritmético a la derecha por rs2." },
  or: { sintaxis: "or rd, rs1, rs2", descripcion: "rd ← rs1 OR rs2." },
  and: { sintaxis: "and rd, rs1, rs2", descripcion: "rd ← rs1 AND rs2." },
  fence: { sintaxis: "fence", descripcion: "Barrera de memoria. En EmuRV (mono-hart) no tiene efecto observable." },
  ecall: { sintaxis: "ecall", descripcion: "Llamada al sistema: el servicio se elige con a7 (ver la Ayuda)." },
  ebreak: { sintaxis: "ebreak", descripcion: "Pausa por software (como un breakpoint en el código); se reanuda con Ejecutar/Paso." },
  // ---- RV32M ----
  mul: { sintaxis: "mul rd, rs1, rs2", descripcion: "rd ← parte baja de rs1 × rs2." },
  mulh: { sintaxis: "mulh rd, rs1, rs2", descripcion: "Parte alta del producto con signo × con signo." },
  mulhsu: { sintaxis: "mulhsu rd, rs1, rs2", descripcion: "Parte alta del producto con signo × sin signo." },
  mulhu: { sintaxis: "mulhu rd, rs1, rs2", descripcion: "Parte alta del producto sin signo × sin signo." },
  div: { sintaxis: "div rd, rs1, rs2", descripcion: "División entera con signo (rd ← rs1 / rs2)." },
  divu: { sintaxis: "divu rd, rs1, rs2", descripcion: "División entera sin signo." },
  rem: { sintaxis: "rem rd, rs1, rs2", descripcion: "Resto de la división con signo." },
  remu: { sintaxis: "remu rd, rs1, rs2", descripcion: "Resto de la división sin signo." },
  // ---- RV64IM (RP32): solo en arquitectura RV64 ----
  ld: { sintaxis: "ld rd, imm(rs1)", descripcion: "Carga un doubleword (8 bytes) desde memoria a rd. Solo RV64." },
  lwu: { sintaxis: "lwu rd, imm(rs1)", descripcion: "Carga una palabra (4 bytes) SIN signo (extiende con ceros a 64 bits). Solo RV64." },
  sd: { sintaxis: "sd rs2, imm(rs1)", descripcion: "Guarda un doubleword (8 bytes) en memoria. Solo RV64." },
  addiw: { sintaxis: "addiw rd, rs1, imm", descripcion: "Suma de 32 bits: opera en los 32 bajos y sign-extiende el resultado a 64. Solo RV64." },
  slliw: { sintaxis: "slliw rd, rs1, shamt", descripcion: "Corrimiento lógico a la izquierda de 32 bits, resultado sign-extendido a 64. Solo RV64." },
  srliw: { sintaxis: "srliw rd, rs1, shamt", descripcion: "Corrimiento lógico a la derecha de 32 bits (entra 0), sign-extendido a 64. Solo RV64." },
  sraiw: { sintaxis: "sraiw rd, rs1, shamt", descripcion: "Corrimiento aritmético a la derecha de 32 bits, sign-extendido a 64. Solo RV64." },
  addw: { sintaxis: "addw rd, rs1, rs2", descripcion: "Suma de 32 bits, resultado sign-extendido a 64. Solo RV64." },
  subw: { sintaxis: "subw rd, rs1, rs2", descripcion: "Resta de 32 bits, resultado sign-extendido a 64. Solo RV64." },
  sllw: { sintaxis: "sllw rd, rs1, rs2", descripcion: "Corrimiento izquierdo de 32 bits, sign-extendido a 64. Solo RV64." },
  srlw: { sintaxis: "srlw rd, rs1, rs2", descripcion: "Corrimiento lógico derecho de 32 bits, sign-extendido a 64. Solo RV64." },
  sraw: { sintaxis: "sraw rd, rs1, rs2", descripcion: "Corrimiento aritmético derecho de 32 bits, sign-extendido a 64. Solo RV64." },
  mulw: { sintaxis: "mulw rd, rs1, rs2", descripcion: "Producto de 32 bits (parte baja), sign-extendido a 64. Solo RV64." },
  divw: { sintaxis: "divw rd, rs1, rs2", descripcion: "División entera con signo de 32 bits, sign-extendida a 64. Solo RV64." },
  divuw: { sintaxis: "divuw rd, rs1, rs2", descripcion: "División entera sin signo de 32 bits, sign-extendida a 64. Solo RV64." },
  remw: { sintaxis: "remw rd, rs1, rs2", descripcion: "Resto con signo de 32 bits, sign-extendido a 64. Solo RV64." },
  remuw: { sintaxis: "remuw rd, rs1, rs2", descripcion: "Resto sin signo de 32 bits, sign-extendido a 64. Solo RV64." },
  // ---- Zicsr (RP8) ----
  csrrw: { sintaxis: "csrrw rd, csr, rs1", descripcion: "Intercambia: rd ← csr, csr ← rs1." },
  csrrs: { sintaxis: "csrrs rd, csr, rs1", descripcion: "rd ← csr; pone a 1 en csr los bits de rs1." },
  csrrc: { sintaxis: "csrrc rd, csr, rs1", descripcion: "rd ← csr; pone a 0 en csr los bits de rs1." },
  csrrwi: { sintaxis: "csrrwi rd, csr, imm", descripcion: "Como csrrw con un inmediato de 5 bits." },
  csrrsi: { sintaxis: "csrrsi rd, csr, imm", descripcion: "Como csrrs con un inmediato de 5 bits." },
  csrrci: { sintaxis: "csrrci rd, csr, imm", descripcion: "Como csrrc con un inmediato de 5 bits." },
  mret: { sintaxis: "mret", descripcion: "Vuelve de un manejador de excepciones: pc ← mepc." },
  // ---- RV32F (RP12) ----
  flw: { sintaxis: "flw frd, imm(rs1)", descripcion: "Carga un float (32 bits) desde memoria a un registro f." },
  fsw: { sintaxis: "fsw frs2, imm(rs1)", descripcion: "Guarda un float (32 bits) en memoria." },
  "fadd.s": { sintaxis: "fadd.s frd, frs1, frs2", descripcion: "Suma de floats (single)." },
  "fsub.s": { sintaxis: "fsub.s frd, frs1, frs2", descripcion: "Resta de floats (single)." },
  "fmul.s": { sintaxis: "fmul.s frd, frs1, frs2", descripcion: "Multiplicación de floats (single)." },
  "fdiv.s": { sintaxis: "fdiv.s frd, frs1, frs2", descripcion: "División de floats (single)." },
  "fsqrt.s": { sintaxis: "fsqrt.s frd, frs1", descripcion: "Raíz cuadrada de un float (single)." },
  "fmin.s": { sintaxis: "fmin.s frd, frs1, frs2", descripcion: "Mínimo de dos floats (single)." },
  "fmax.s": { sintaxis: "fmax.s frd, frs1, frs2", descripcion: "Máximo de dos floats (single)." },
  "fmadd.s": { sintaxis: "fmadd.s frd, f1, f2, f3", descripcion: "Multiplicar-sumar: frd ← f1×f2 + f3 (single)." },
  "fmsub.s": { sintaxis: "fmsub.s frd, f1, f2, f3", descripcion: "frd ← f1×f2 − f3 (single)." },
  "fnmadd.s": { sintaxis: "fnmadd.s frd, f1, f2, f3", descripcion: "frd ← −(f1×f2) − f3 (single)." },
  "fnmsub.s": { sintaxis: "fnmsub.s frd, f1, f2, f3", descripcion: "frd ← −(f1×f2) + f3 (single)." },
  "fcvt.w.s": { sintaxis: "fcvt.w.s rd, frs1", descripcion: "Convierte float a entero con signo (a un registro x)." },
  "fcvt.wu.s": { sintaxis: "fcvt.wu.s rd, frs1", descripcion: "Convierte float a entero sin signo." },
  "fcvt.s.w": { sintaxis: "fcvt.s.w frd, rs1", descripcion: "Convierte entero con signo a float." },
  "fcvt.s.wu": { sintaxis: "fcvt.s.wu frd, rs1", descripcion: "Convierte entero sin signo a float." },
  "fmv.x.w": { sintaxis: "fmv.x.w rd, frs1", descripcion: "Copia los bits crudos de un registro f a uno x (sin convertir)." },
  "fmv.w.x": { sintaxis: "fmv.w.x frd, rs1", descripcion: "Copia los bits crudos de un registro x a uno f (sin convertir)." },
  "feq.s": { sintaxis: "feq.s rd, frs1, frs2", descripcion: "rd ← 1 si los floats son iguales, si no 0." },
  "flt.s": { sintaxis: "flt.s rd, frs1, frs2", descripcion: "rd ← 1 si frs1 < frs2 (floats)." },
  "fle.s": { sintaxis: "fle.s rd, frs1, frs2", descripcion: "rd ← 1 si frs1 <= frs2 (floats)." },
  "fsgnj.s": { sintaxis: "fsgnj.s frd, frs1, frs2", descripcion: "frd ← frs1 con el signo de frs2." },
  "fsgnjn.s": { sintaxis: "fsgnjn.s frd, frs1, frs2", descripcion: "frd ← frs1 con el signo OPUESTO de frs2." },
  "fsgnjx.s": { sintaxis: "fsgnjx.s frd, frs1, frs2", descripcion: "frd ← frs1 con el signo = XOR de los signos." },
  "fclass.s": { sintaxis: "fclass.s rd, frs1", descripcion: "Clasifica el float (cero, infinito, NaN…) en una máscara de bits." },
  // ---- RV32D (RP12) ----
  fld: { sintaxis: "fld frd, imm(rs1)", descripcion: "Carga un double (64 bits) desde memoria." },
  fsd: { sintaxis: "fsd frs2, imm(rs1)", descripcion: "Guarda un double (64 bits) en memoria." },
  "fadd.d": { sintaxis: "fadd.d frd, frs1, frs2", descripcion: "Suma de doubles." },
  "fsub.d": { sintaxis: "fsub.d frd, frs1, frs2", descripcion: "Resta de doubles." },
  "fmul.d": { sintaxis: "fmul.d frd, frs1, frs2", descripcion: "Multiplicación de doubles." },
  "fdiv.d": { sintaxis: "fdiv.d frd, frs1, frs2", descripcion: "División de doubles." },
  "fsqrt.d": { sintaxis: "fsqrt.d frd, frs1", descripcion: "Raíz cuadrada de un double." },
  "fmin.d": { sintaxis: "fmin.d frd, frs1, frs2", descripcion: "Mínimo de dos doubles." },
  "fmax.d": { sintaxis: "fmax.d frd, frs1, frs2", descripcion: "Máximo de dos doubles." },
  "fmadd.d": { sintaxis: "fmadd.d frd, f1, f2, f3", descripcion: "frd ← f1×f2 + f3 (double)." },
  "fmsub.d": { sintaxis: "fmsub.d frd, f1, f2, f3", descripcion: "frd ← f1×f2 − f3 (double)." },
  "fnmadd.d": { sintaxis: "fnmadd.d frd, f1, f2, f3", descripcion: "frd ← −(f1×f2) − f3 (double)." },
  "fnmsub.d": { sintaxis: "fnmsub.d frd, f1, f2, f3", descripcion: "frd ← −(f1×f2) + f3 (double)." },
  "fcvt.w.d": { sintaxis: "fcvt.w.d rd, frs1", descripcion: "Convierte double a entero con signo." },
  "fcvt.wu.d": { sintaxis: "fcvt.wu.d rd, frs1", descripcion: "Convierte double a entero sin signo." },
  "fcvt.d.w": { sintaxis: "fcvt.d.w frd, rs1", descripcion: "Convierte entero con signo a double." },
  "fcvt.d.wu": { sintaxis: "fcvt.d.wu frd, rs1", descripcion: "Convierte entero sin signo a double." },
  "fcvt.s.d": { sintaxis: "fcvt.s.d frd, frs1", descripcion: "Convierte double a float (single)." },
  "fcvt.d.s": { sintaxis: "fcvt.d.s frd, frs1", descripcion: "Convierte float (single) a double." },
  "feq.d": { sintaxis: "feq.d rd, frs1, frs2", descripcion: "rd ← 1 si los doubles son iguales." },
  "flt.d": { sintaxis: "flt.d rd, frs1, frs2", descripcion: "rd ← 1 si frs1 < frs2 (doubles)." },
  "fle.d": { sintaxis: "fle.d rd, frs1, frs2", descripcion: "rd ← 1 si frs1 <= frs2 (doubles)." },
  "fsgnj.d": { sintaxis: "fsgnj.d frd, frs1, frs2", descripcion: "frd ← frs1 con el signo de frs2 (double)." },
  "fsgnjn.d": { sintaxis: "fsgnjn.d frd, frs1, frs2", descripcion: "frd ← frs1 con el signo opuesto de frs2 (double)." },
  "fsgnjx.d": { sintaxis: "fsgnjx.d frd, frs1, frs2", descripcion: "frd ← frs1 con el signo = XOR de los signos (double)." },
  "fclass.d": { sintaxis: "fclass.d rd, frs1", descripcion: "Clasifica el double (cero, infinito, NaN…)." },
  // ---- RV32A (RP13) — formas base ----
  "lr.w": { sintaxis: "lr.w rd, (rs1)", descripcion: "Load-reserved: lee la palabra y reserva la celda (mono-hart: ver docs)." },
  "sc.w": { sintaxis: "sc.w rd, rs2, (rs1)", descripcion: "Store-conditional: escribe si la reserva sigue válida; rd ← 0 en éxito (en EmuRV siempre)." },
  "amoswap.w": { sintaxis: "amoswap.w rd, rs2, (rs1)", descripcion: "Intercambia atómicamente la memoria con rs2; rd ← valor viejo." },
  "amoadd.w": { sintaxis: "amoadd.w rd, rs2, (rs1)", descripcion: "Suma atómica: memoria += rs2; rd ← valor viejo." },
  "amoand.w": { sintaxis: "amoand.w rd, rs2, (rs1)", descripcion: "AND atómico con rs2; rd ← valor viejo." },
  "amoor.w": { sintaxis: "amoor.w rd, rs2, (rs1)", descripcion: "OR atómico con rs2; rd ← valor viejo." },
  "amoxor.w": { sintaxis: "amoxor.w rd, rs2, (rs1)", descripcion: "XOR atómico con rs2; rd ← valor viejo." },
  "amomin.w": { sintaxis: "amomin.w rd, rs2, (rs1)", descripcion: "Mínimo atómico con signo; rd ← valor viejo." },
  "amomax.w": { sintaxis: "amomax.w rd, rs2, (rs1)", descripcion: "Máximo atómico con signo; rd ← valor viejo." },
  "amominu.w": { sintaxis: "amominu.w rd, rs2, (rs1)", descripcion: "Mínimo atómico sin signo; rd ← valor viejo." },
  "amomaxu.w": { sintaxis: "amomaxu.w rd, rs2, (rs1)", descripcion: "Máximo atómico sin signo; rd ← valor viejo." },
  // ---- RV32C (RP13) ----
  "c.nop": { sintaxis: "c.nop", descripcion: "No hace nada (versión comprimida de 2 bytes)." },
  "c.addi": { sintaxis: "c.addi rd, imm", descripcion: "rd ← rd + inmediato (6 bits). Comprimida (2 bytes)." },
  "c.li": { sintaxis: "c.li rd, imm", descripcion: "rd ← inmediato (6 bits con signo). Comprimida." },
  "c.mv": { sintaxis: "c.mv rd, rs2", descripcion: "rd ← rs2. Comprimida." },
  "c.jr": { sintaxis: "c.jr rs1", descripcion: "Salta a la dirección en rs1. Comprimida." },
  "c.j": { sintaxis: "c.j etiqueta", descripcion: "Salto incondicional. Comprimida." },
  "c.lw": { sintaxis: "c.lw rd, imm(rs1)", descripcion: "Carga palabra; registros x8–x15. Comprimida." },
  "c.sw": { sintaxis: "c.sw rs2, imm(rs1)", descripcion: "Guarda palabra; registros x8–x15. Comprimida." },
  "c.beqz": { sintaxis: "c.beqz rs1, etiqueta", descripcion: "Salta si rs1 == 0; rs1 en x8–x15. Comprimida." },
  "c.bnez": { sintaxis: "c.bnez rs1, etiqueta", descripcion: "Salta si rs1 != 0; rs1 en x8–x15. Comprimida." },
  // ---- pseudo-instrucciones ----
  nop: { sintaxis: "nop", descripcion: "No hace nada (addi zero, zero, 0)." },
  li: { sintaxis: "li rd, imm", descripcion: "Carga un inmediato cualquiera en rd (se expande a 1 o 2 instrucciones)." },
  la: { sintaxis: "la rd, etiqueta", descripcion: "Carga en rd la dirección de una etiqueta." },
  lla: { sintaxis: "lla rd, etiqueta", descripcion: "Como la (dirección local de una etiqueta)." },
  mv: { sintaxis: "mv rd, rs1", descripcion: "Copia rs1 en rd (addi rd, rs1, 0)." },
  not: { sintaxis: "not rd, rs1", descripcion: "rd ← NOT rs1 (xori rd, rs1, -1)." },
  neg: { sintaxis: "neg rd, rs1", descripcion: "rd ← −rs1 (sub rd, zero, rs1)." },
  seqz: { sintaxis: "seqz rd, rs1", descripcion: "rd ← 1 si rs1 == 0." },
  snez: { sintaxis: "snez rd, rs1", descripcion: "rd ← 1 si rs1 != 0." },
  sltz: { sintaxis: "sltz rd, rs1", descripcion: "rd ← 1 si rs1 < 0." },
  sgtz: { sintaxis: "sgtz rd, rs1", descripcion: "rd ← 1 si rs1 > 0." },
  beqz: { sintaxis: "beqz rs1, etiqueta", descripcion: "Salta si rs1 == 0." },
  bnez: { sintaxis: "bnez rs1, etiqueta", descripcion: "Salta si rs1 != 0." },
  blez: { sintaxis: "blez rs1, etiqueta", descripcion: "Salta si rs1 <= 0." },
  bgez: { sintaxis: "bgez rs1, etiqueta", descripcion: "Salta si rs1 >= 0." },
  bltz: { sintaxis: "bltz rs1, etiqueta", descripcion: "Salta si rs1 < 0." },
  bgtz: { sintaxis: "bgtz rs1, etiqueta", descripcion: "Salta si rs1 > 0." },
  bgt: { sintaxis: "bgt rs1, rs2, etiqueta", descripcion: "Salta si rs1 > rs2 (con signo)." },
  ble: { sintaxis: "ble rs1, rs2, etiqueta", descripcion: "Salta si rs1 <= rs2 (con signo)." },
  bgtu: { sintaxis: "bgtu rs1, rs2, etiqueta", descripcion: "Salta si rs1 > rs2 (sin signo)." },
  bleu: { sintaxis: "bleu rs1, rs2, etiqueta", descripcion: "Salta si rs1 <= rs2 (sin signo)." },
  j: { sintaxis: "j etiqueta", descripcion: "Salto incondicional a la etiqueta." },
  jr: { sintaxis: "jr rs1", descripcion: "Salta a la dirección en rs1." },
  ret: { sintaxis: "ret", descripcion: "Retorna de una función (jr ra)." },
  call: { sintaxis: "call etiqueta", descripcion: "Llama a una función (guarda el retorno en ra)." },
  tail: { sintaxis: "tail etiqueta", descripcion: "Salto de cola a una función (sin guardar retorno)." },
  csrr: { sintaxis: "csrr rd, csr", descripcion: "Lee un CSR: rd ← csr." },
  csrw: { sintaxis: "csrw csr, rs1", descripcion: "Escribe un CSR: csr ← rs1." },
  csrs: { sintaxis: "csrs csr, rs1", descripcion: "Pone a 1 en el CSR los bits de rs1." },
  csrc: { sintaxis: "csrc csr, rs1", descripcion: "Pone a 0 en el CSR los bits de rs1." },
  "fmv.s": { sintaxis: "fmv.s frd, frs1", descripcion: "Copia un float (fsgnj.s frd, frs1, frs1)." },
  "fmv.d": { sintaxis: "fmv.d frd, frs1", descripcion: "Copia un double." },
  "fabs.s": { sintaxis: "fabs.s frd, frs1", descripcion: "Valor absoluto de un float." },
  "fabs.d": { sintaxis: "fabs.d frd, frs1", descripcion: "Valor absoluto de un double." },
  "fneg.s": { sintaxis: "fneg.s frd, frs1", descripcion: "Cambia el signo de un float." },
  "fneg.d": { sintaxis: "fneg.d frd, frs1", descripcion: "Cambia el signo de un double." },
};

/// Sufijos de ordenamiento de las atómicas (RV32A): se quitan para buscar
/// la entrada base (amoadd.w.aqrl → amoadd.w).
const SUFIJOS_AMO = [".aqrl", ".aq", ".rl"];

/// Devuelve la ayuda de un mnemónico (tolerando sufijos de A), o undefined
/// si no hay entrada — en cuyo caso NO se muestra tooltip.
export function ayudaDe(palabra: string): AyudaInstr | undefined {
  const w = palabra.toLowerCase();
  const directa = AYUDA_ISA[w];
  if (directa) return directa;
  for (const s of SUFIJOS_AMO) {
    if (w.endsWith(s)) {
      const base = AYUDA_ISA[w.slice(0, -s.length)];
      if (base) return base;
    }
  }
  return undefined;
}

/// Texto plano para el `info` del autocompletado (sintaxis — descripción).
export function descripcionInstr(mnem: string): string | undefined {
  const a = ayudaDe(mnem);
  return a ? `${a.sintaxis} — ${a.descripcion}` : undefined;
}
