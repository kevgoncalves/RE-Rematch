// DLL exposing the Oodle interface that Oodle.NET (used by CUE4Parse) loads, on top of the open-source ooz decoder.
// Decompression only. Compress returns 0 (failure) and exists only because Oodle.NET requires the symbol.
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

int Kraken_Decompress(const unsigned char *src, size_t src_len, unsigned char *dst, size_t dst_len);

#define EXPORT extern "C" __declspec(dllexport)

EXPORT int64_t OodleLZ_Decompress(const void *comp, int64_t comp_len, void *raw, int64_t raw_len,
                                  int, int, int, void *, int64_t, void *, void *, void *, int64_t, int) {
  if (!comp || !raw || comp_len <= 0 || raw_len <= 0 || raw_len > INT32_MAX)
    return 0;
  // ooz writes up to 64 bytes past the end of the buffer, so decode into a temporary one
  unsigned char *tmp = (unsigned char *)malloc((size_t)raw_len + 64);
  if (!tmp)
    return 0;
  int n = Kraken_Decompress((const unsigned char *)comp, (size_t)comp_len, tmp, (size_t)raw_len);
  if (n == raw_len)
    memcpy(raw, tmp, (size_t)raw_len);
  free(tmp);
  return n == raw_len ? raw_len : 0;
}

EXPORT int64_t OodleLZ_Compress(int, const void *, int64_t, void *, int, const void *, const void *, const void *, void *, int64_t) {
  return 0;
}

EXPORT int64_t OodleLZ_GetCompressedBufferSizeNeeded(int, int64_t raw_size) {
  return raw_size + 274 * ((raw_size + 0x3FFFF) / 0x40000) + 1024;
}
