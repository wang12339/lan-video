const SHA256_K = new Uint32Array([
  0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
  0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
  0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
  0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
  0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
  0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
  0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
  0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
])

const SHA256_INIT = new Uint32Array([
  0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
])

function rotr(x: number, n: number): number {
  return (x >>> n) | (x << (32 - n))
}

export class Sha256 {
  private h: Uint32Array
  private buffer = new Uint8Array(64)
  private bufferLen = 0
  private totalBytes = 0

  constructor() {
    this.h = new Uint32Array(SHA256_INIT)
  }

  update(data: Uint8Array): void {
    this.totalBytes += data.length
    let pos = 0
    if (this.bufferLen > 0) {
      const need = 64 - this.bufferLen
      const take = Math.min(need, data.length)
      this.buffer.set(data.subarray(0, take), this.bufferLen)
      this.bufferLen += take
      pos = take
      if (this.bufferLen === 64) {
        this.compress(this.buffer)
        this.bufferLen = 0
      }
    }
    while (pos + 64 <= data.length) {
      this.compress(data.subarray(pos, pos + 64))
      pos += 64
    }
    if (pos < data.length) {
      this.buffer.set(data.subarray(pos))
      this.bufferLen = data.length - pos
    }
  }

  private compress(block: Uint8Array): void {
    const w = new Uint32Array(64)
    for (let i = 0; i < 16; i++) {
      const o = i * 4
      w[i] = (block[o]! << 24) | (block[o + 1]! << 16) | (block[o + 2]! << 8) | block[o + 3]!
    }
    for (let i = 16; i < 64; i++) {
      const w15 = w[i - 15]!
      const w2 = w[i - 2]!
      const s0 = rotr(w15, 7) ^ rotr(w15, 18) ^ (w15 >>> 3)
      const s1 = rotr(w2, 17) ^ rotr(w2, 19) ^ (w2 >>> 10)
      w[i] = (w[i - 16]! + s0 + w[i - 7]! + s1) | 0
    }
    const h = this.h
    let a = h[0]!
    let b = h[1]!
    let c = h[2]!
    let d = h[3]!
    let e = h[4]!
    let f = h[5]!
    let g = h[6]!
    let hh = h[7]!
    for (let i = 0; i < 64; i++) {
      const S1 = rotr(e, 6) ^ rotr(e, 11) ^ rotr(e, 25)
      const ch = (e & f) ^ (~e & g)
      const temp1 = (hh + S1 + ch + SHA256_K[i]! + w[i]!) | 0
      const S0 = rotr(a, 2) ^ rotr(a, 13) ^ rotr(a, 22)
      const maj = (a & b) ^ (a & c) ^ (b & c)
      const temp2 = (S0 + maj) | 0
      hh = g
      g = f
      f = e
      e = (d + temp1) | 0
      d = c
      c = b
      b = a
      a = (temp1 + temp2) | 0
    }
    h[0] = (h[0]! + a) | 0
    h[1] = (h[1]! + b) | 0
    h[2] = (h[2]! + c) | 0
    h[3] = (h[3]! + d) | 0
    h[4] = (h[4]! + e) | 0
    h[5] = (h[5]! + f) | 0
    h[6] = (h[6]! + g) | 0
    h[7] = (h[7]! + hh) | 0
  }

  digest(): string {
    const bitLen = this.totalBytes * 8
    const hi = Math.floor(bitLen / 0x100000000)
    const lo = bitLen % 0x100000000
    const padLen = this.bufferLen < 56 ? 64 - this.bufferLen : 128 - this.bufferLen
    const padded = new Uint8Array(padLen)
    padded[0] = 0x80
    padded[padLen - 8] = (hi >>> 24) & 0xff
    padded[padLen - 7] = (hi >>> 16) & 0xff
    padded[padLen - 6] = (hi >>> 8) & 0xff
    padded[padLen - 5] = hi & 0xff
    padded[padLen - 4] = (lo >>> 24) & 0xff
    padded[padLen - 3] = (lo >>> 16) & 0xff
    padded[padLen - 2] = (lo >>> 8) & 0xff
    padded[padLen - 1] = lo & 0xff
    this.update(padded)
    let out = ''
    for (let i = 0; i < 8; i++) {
      out += this.h[i]!.toString(16).padStart(8, '0')
    }
    return out
  }
}
