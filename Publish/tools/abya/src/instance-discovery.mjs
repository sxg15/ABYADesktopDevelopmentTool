import path from 'node:path';

// Multiple records can survive a host restart or PID reuse. Only status reads
// may disambiguate them; never retry a capability or choose by file timestamp.
export async function liveCandidates(candidates, env, request) {
  const result = [];
  for (let offset = 0; offset < candidates.length; offset += 4) {
    const batch = await Promise.all(candidates.slice(offset, offset + 4).map(async candidate => {
      try {
        const response = await request(candidate, 'status', {}, {
          signal: AbortSignal.timeout(1500),
        });
        if (response.success !== true) return null;
        const block = response.content?.find(item => item.type === 'text');
        const identity = JSON.parse(block?.text || 'null');
        if (!identity || identity.processId !== candidate.processId ||
            !['Player', 'EditorDev'].includes(identity.target) ||
            typeof identity.projectPath !== 'string' ||
            path.resolve(identity.projectPath).toLowerCase() !==
              path.resolve(candidate.projectPath).toLowerCase() ||
            (env.ABYA_CLI_DESKTOP_INSTANCE_ID &&
              identity.desktopInstanceId !== env.ABYA_CLI_DESKTOP_INSTANCE_ID)) return null;
        return candidate;
      } catch { return null; } // Stale, unreachable, unauthenticated or foreign host.
    }));
    result.push(...batch.filter(Boolean));
  }
  return result;
}
