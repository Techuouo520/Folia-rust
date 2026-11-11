/*
 * @name HTTP fixture
 */
lx.on('request', ({ info }) => new Promise((resolve, reject) => {
  const options = info.musicInfo.options || {};
  if (options.fixtureOrder) options.body = { z: 1, a: 2 };
  lx.request(info.musicInfo.url, options, (error, response, body) => {
    if (error) return reject(error);
    if (response.statusCode !== 201 || response.statusMessage !== 'Created') return reject(new Error('status'));
    if (response.body !== body || body.ok !== true || !(response.raw instanceof Uint8Array)) return reject(new Error('body'));
    if (response.bytes !== response.raw.length || response.headers['x-fixture'] !== 'yes') return reject(new Error('headers'));
    resolve('https://example.invalid/http-ok');
  });
}));
lx.send('inited', { sources: { wy: { type: 'music', actions: ['musicUrl'], qualitys: ['128k'] } } });
