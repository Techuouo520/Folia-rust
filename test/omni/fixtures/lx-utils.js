/*
 * @name Utility fixture
 */
const { buffer: b, crypto: c, zlib: z } = lx.utils;
const check = (actual, expected) => { if (actual !== expected) throw new Error(String(actual)); };
lx.on('request', async () => {
  check(typeof process, 'undefined'); check(typeof require, 'undefined');
  check(typeof std, 'undefined'); check(typeof __lxNative, 'undefined');
  check(lx.env, 'desktop'); check(lx.version, '2.0.0'); check(lx.currentScriptInfo.name, 'Utility fixture');
  check(c.md5('abc'), '900150983cd24fb0d6963f7d28e17f72');
  check(b.bufToString(b.from('中文'), 'hex'), 'e4b8ade69687');
  check(b.bufToString(b.from('YWJj', 'base64')), 'abc');
  check(b.bufToString(b.from('616263', 'hex')), 'abc');
  check(b.bufToString(b.from('AB', 'utf16le'), 'hex'), '41004200');
  check(b.bufToString(b.from([255, 65]), 'ascii'), '\x7fA');
  check(b.bufToString(b.from(new Uint8Array([7, 8, 9]).buffer, 1, 1), 'hex'), '08');
  check(b.bufToString(c.aesEncrypt(b.from('6bc1bee22e409f96e93d7e117393172a', 'hex'), 'aes-128-cbc', b.from('2b7e151628aed2a6abf7158809cf4f3c', 'hex'), b.from('000102030405060708090a0b0c0d0e0f', 'hex')), 'hex').slice(0, 32), '7649abac8119b246cee98e9b12e9197d');
  check(b.bufToString(c.aesEncrypt(b.from('00112233445566778899aabbccddeeff', 'hex'), 'aes-128-ecb', b.from('000102030405060708090a0b0c0d0e0f', 'hex'), null), 'hex').slice(0, 32), '69c4e0d86a7b0430d8cdb78070b4c55a');
  check(b.bufToString(c.aesEncrypt(b.from('6bc1bee22e409f96e93d7e117393172a', 'hex'), 'aes-128-ctr', b.from('2b7e151628aed2a6abf7158809cf4f3c', 'hex'), b.from('f0f1f2f3f4f5f6f7f8f9fafbfcfdfeff', 'hex')), 'hex'), '874d6191b620e3261bef6864990db6ce');
  check(c.randomBytes(16).length, 16);
  check(b.bufToString(await z.inflate(b.from('789ccb48cdc9c90700062c0215', 'hex'))), 'hello');
  check(b.bufToString(await z.deflate(b.from('hello')), 'hex'), '789ccb48cdc9c90700062c0215');
  check(b.bufToString(await z.inflate(await z.deflate(b.from('hello')))), 'hello');
  return 'https://example.invalid/utils-ok';
});
lx.send('inited', { sources: { wy: { type: 'music', actions: ['musicUrl'], qualitys: ['128k'] } } });
