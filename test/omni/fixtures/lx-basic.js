/*
 * @name Folia fixture
 * @description Synthetic source used only in tests
 * @version 1.2.3
 * @author Folia tests
 * @homepage https://example.invalid
 */
lx.on(lx.EVENT_NAMES.request, async ({ source, action, info }) => {
  if (action !== 'musicUrl' || !info.musicInfo || source !== 'wy') throw new Error('bad request');
  if (info.musicInfo.name === 'reject') throw new Error('fixture rejection');
  if (info.musicInfo.name === 'loop') while (true) {}
  if (info.musicInfo.name === 'pending') return new Promise(() => {});
  if (info.musicInfo.name === 'memory') { const a = []; while (true) a.push(new Uint8Array(1024 * 1024)); }
  return info.musicInfo.url || 'https://example.invalid/audio.mp3';
});
lx.send(lx.EVENT_NAMES.inited, { sources: {
  wy: { name: 'NetEase', type: 'music', actions: ['musicUrl', 'invalid'], qualitys: ['128k', '320k', 'flac', 'invalid'] },
  bogus: { type: 'music', actions: ['musicUrl'], qualitys: ['128k'] },
  kw: { type: 'other', actions: ['musicUrl'], qualitys: ['128k'] },
  local: { type: 'music', actions: ['musicUrl', 'pic', 'lyric'], qualitys: [] },
}});
