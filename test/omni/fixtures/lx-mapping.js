/*
 * @name Mapping fixture
 */
lx.on('request', async ({source, info}) => {
  const m = info.musicInfo;
  return 'https://example.invalid/' + [source, m.songmid, m.songId || '', m.strMediaMid || '', m.albumId, m.hash || '', m._types['320k']?.hash || '', info.type].join('/');
});
lx.send('inited', {sources: Object.fromEntries(['wy', 'kg', 'tx'].map(id => [id, {type:'music', actions:['musicUrl'], qualitys:['128k','320k','flac','flac24bit']}]))});
