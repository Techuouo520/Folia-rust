// Sandboxed LX desktop API; native functions are captured and removed before user code runs.
(function (native, info) {
  'use strict';
  delete globalThis.__lxNative;
  delete globalThis.__lxInfo;
  const callbacks = new Map();
  const timers = new Map();
  let nextId = 0;
  let handler = null;
  let initialized = false;
  let alerted = false;
  let initialization = null;
  let update = null;
  const cut = (text, length) => text.slice(0, length).replace(/[\uD800-\uDBFF]$/, '\uFFFD');
  const invoke = (op, args) => {
    const reply = JSON.parse(native(op, JSON.stringify(args)));
    if (reply.error) throw new Error(reply.error);
    return reply.value;
  };
  const bytes = value => {
    if (typeof value === 'string') return invoke('encode', [value, 'utf8']);
    if (value instanceof ArrayBuffer) return Array.from(new Uint8Array(value));
    if (ArrayBuffer.isView(value)) return Array.from(value);
    if (Array.isArray(value)) return value.map(n => Number(n) & 255);
    if (value && value.type === 'Buffer' && Array.isArray(value.data)) return bytes(value.data);
    throw new TypeError('Expected a buffer');
  };
  const buffer = value => new Uint8Array(value);
  const query = (value, prefix = '', depth = 0) => {
    if (depth > 32) throw new Error('Source resource limit reached');
    if (Array.isArray(value)) return value.map(v => query(v, prefix ? prefix + '[]' : '', depth + 1)).join('&');
    if (value && typeof value === 'object') return Object.keys(value).map(key => query(value[key], prefix ? prefix + '[' + encodeURIComponent(key) + ']' : encodeURIComponent(key), depth + 1)).join('&');
    if (prefix) return prefix + '=' + (value == null ? '' : encodeURIComponent(String(value)));
    if (typeof value === 'string' && value.includes('=')) return value;
    throw new TypeError('Cannot build a query string');
  };
  const request = (url, options, callback) => {
    if (typeof callback !== 'function') throw new TypeError('Expected a callback');
    const id = ++nextId;
    const opts = { ...options };
    opts.__raw = opts.body instanceof ArrayBuffer || ArrayBuffer.isView(opts.body);
    if (opts.__raw) opts.body = bytes(opts.body);
    const data = opts.body || opts.form || opts.formData;
    opts.__data = null;
    if (data && !opts.__raw) {
      const contentType = Object.entries(opts.headers || {}).find(([key]) => key.toLowerCase() === 'content-type')?.[1];
      const json = !!opts.body && contentType === 'application/json';
      opts.__data = String(opts.method || 'get').toLowerCase() === 'get' && !json ? query(data)
        : typeof data === 'string' ? data : json ? JSON.stringify(data) : query(data);
    }
    invoke('request', [id, String(url), opts]);
    callbacks.set(id, callback);
    return () => { callbacks.delete(id); invoke('cancel', [id]); };
  };
  const lx = {
    EVENT_NAMES: Object.freeze({ request: 'request', inited: 'inited', updateAlert: 'updateAlert' }),
    version: '2.0.0', env: 'desktop', currentScriptInfo: info,
    request,
    on: async (event, fn) => {
      if (event !== 'request' || typeof fn !== 'function') throw new Error('Unsupported event');
      handler = fn;
    },
    send: async (event, data) => {
      if (event === 'inited') {
        if (initialized) throw new Error('Script is inited');
        initialized = true;
        if (!data || !data.sources) throw new Error('Missing sources');
        const sources = {};
        for (const source of ['kw', 'kg', 'tx', 'wy', 'mg', 'local']) {
          const item = data.sources[source];
          if (!item || item.type !== 'music') continue;
          if (!Array.isArray(item.actions) || !Array.isArray(item.qualitys)) throw new Error('Invalid sources');
          sources[source] = {
            name: typeof item.name === 'string' ? cut(item.name, 56) : '', type: 'music',
            actions: (source === 'local' ? ['musicUrl', 'lyric', 'pic'] : ['musicUrl']).filter(a => item.actions.includes(a)),
            qualitys: (source === 'local' ? [] : ['128k', '320k', 'flac', 'flac24bit']).filter(q => item.qualitys.includes(q)),
          };
        }
        initialization = JSON.stringify(sources);
      } else if (event === 'updateAlert') {
        if (alerted) throw new Error('The update alert can only be called once');
        alerted = true;
        if (!data || typeof data.log !== 'string' || !data.log) throw new Error('log is required');
        update = { log: data.log.length > 1024 ? cut(data.log, 1024) + '...' : data.log,
          updateUrl: typeof data.updateUrl === 'string' && data.updateUrl.length <= 1024 && /^https?:\/\/[^\s]+$/.test(data.updateUrl) ? data.updateUrl : null };
      } else throw new Error('Unsupported event');
    },
    utils: {
      buffer: {
        from(value, encoding, length) {
          if (typeof value === 'string') return buffer(invoke('encode', [value, encoding || 'utf8']));
          if (value instanceof ArrayBuffer) return new Uint8Array(value, encoding || 0, length);
          return buffer(bytes(value));
        },
        bufToString: (value, encoding = 'utf8') => invoke('decode', [bytes(value), encoding]),
      },
      crypto: {
        md5: value => invoke('md5', [bytes(value)]),
        randomBytes: size => buffer(invoke('random', [size])),
        aesEncrypt: (value, mode, key, iv) => buffer(invoke('aes', [bytes(value), mode, bytes(key), iv == null ? [] : bytes(iv)])),
        rsaEncrypt: (value, key) => buffer(invoke('rsa', [bytes(value), key])),
      },
      zlib: {
        inflate: async value => buffer(invoke('inflate', [bytes(value)])),
        deflate: async value => buffer(invoke('deflate', [bytes(value)])),
      },
    },
  };
  globalThis.lx = Object.freeze(lx);
  globalThis.console = Object.freeze(Object.fromEntries(['log', 'info', 'warn', 'error', 'debug'].map(k => [k, () => {}])));
  globalThis.setTimeout = (fn, delay = 0, ...args) => {
    if (typeof fn !== 'function' || timers.size >= 256) throw new Error('Invalid timer');
    const id = ++nextId;
    timers.set(id, { at: Date.now() + Math.max(0, Number(delay) || 0), fn, args });
    return id;
  };
  globalThis.clearTimeout = id => timers.delete(id);
  return Object.freeze({
    pending: () => callbacks.size + timers.size,
    initialized: () => initialization,
    update: () => { const result = JSON.stringify(update); update = null; return result; },
    request: data => {
      if (!handler) throw new Error('Request event is not defined');
      const result = handler.call(lx, JSON.parse(data));
      if (!result || typeof result.then !== 'function') throw new Error('Handler must return a Promise');
      return result;
    },
    response: data => {
      const reply = JSON.parse(data);
      const callback = callbacks.get(reply.id);
      callbacks.delete(reply.id);
      if (!callback) return;
      if (reply.error) callback.call(lx, new Error(reply.error), null, null);
      else {
        reply.value.raw = buffer(reply.value.raw);
        callback.call(lx, null, reply.value, reply.value.body);
      }
    },
    tick: () => {
      const now = Date.now();
      for (const [id, timer] of timers) if (timer.at <= now) {
        timers.delete(id); timer.fn(...timer.args);
      }
    },
  });
})(globalThis.__lxNative, globalThis.__lxInfo)
