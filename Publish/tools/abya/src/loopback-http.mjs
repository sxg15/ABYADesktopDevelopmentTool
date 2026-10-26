import http from 'node:http';

// Node fetch/Undici has its own header/body deadlines. Runtime jobs may legitimately
// exceed those defaults; this transport uses the caller's single bounded deadline.
export function loopbackFetch(address, options = {}) {
  const url = new URL(address);
  if (url.protocol !== 'http:' || url.hostname !== '127.0.0.1' || url.username || url.password)
    return Promise.reject(new Error('Runtime transport requires loopback HTTP'));
  return new Promise((resolve, reject) => {
    const req = http.request(url, {
      method: options.method ?? 'POST', headers: options.headers,
      agent: false, signal: options.signal,
    }, res => {
      const chunks = []; let length = 0;
      res.on('data', chunk => {
        length += chunk.length;
        if (length > 32 * 1024 * 1024) {
          res.destroy(new Error('Runtime response exceeds 32 MiB'));
          return;
        }
        chunks.push(chunk);
      });
      res.on('error', reject);
      res.on('aborted', () => reject(new Error('Runtime response disconnected before completion')));
      res.on('end', () => resolve({
        ok: res.statusCode >= 200 && res.statusCode < 300, status: res.statusCode,
        json: async () => JSON.parse(Buffer.concat(chunks).toString('utf8')),
      }));
    });
    // No redirects, proxy inheritance, idle socket timeout or automatic replay.
    req.on('error', reject);
    req.end(options.body);
  });
}
