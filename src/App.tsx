import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';

interface DnsProfile {
  name: string;
  primary: string;
  secondary: string;
}

interface AppStatus {
  zapret_running: boolean;
  is_admin: boolean;
  current_dns: string;
}

type ZapretMode = 'best' | 'all' | 'default' | 'custom';

const ZAPRET_MODES: { id: ZapretMode; label: string; desc: string }[] = [
  { id: 'best', label: 'BEST', desc: 'Optimized for speed & compatibility' },
  { id: 'all', label: 'ALL', desc: 'Maximum bypass coverage' },
  { id: 'default', label: 'DEFAULT', desc: 'Standard fake+multidisorder' },
  { id: 'custom', label: 'CUSTOM', desc: 'Use bundled bat configuration' },
];

function App() {
  const [status, setStatus] = useState<AppStatus | null>(null);
  const [dnsProfiles, setDnsProfiles] = useState<DnsProfile[]>([]);
  const [selectedDns, setSelectedDns] = useState('Cloudflare');
  const [zapretMode, setZapretMode] = useState<ZapretMode>('best');
  const [loading, setLoading] = useState(false);
  const [toast, setToast] = useState<{ msg: string; type: 'ok' | 'err' } | null>(null);

  useEffect(() => {
    loadData();
    const interval = setInterval(loadData, 3000);
    return () => clearInterval(interval);
  }, []);

  const loadData = async () => {
    try {
      const profiles = await invoke<DnsProfile[]>('get_dns_options');
      setDnsProfiles(profiles);
      const s = await invoke<AppStatus>('check_status');
      setStatus(s);
    } catch {}
  };

  const notify = (msg: string, type: 'ok' | 'err') => {
    setToast({ msg, type });
    setTimeout(() => setToast(null), 3000);
  };

  const toggleZapret = async () => {
    if (!status) return;
    setLoading(true);
    try {
      const res = await invoke<string>('toggle_zapret', {
        enable: !status.zapret_running,
        mode: zapretMode,
      });
      notify(res, 'ok');
      await loadData();
    } catch (e) {
      notify(String(e), 'err');
    } finally {
      setLoading(false);
    }
  };

  const applyDns = async () => {
    setLoading(true);
    try {
      const res = await invoke<string>('set_dns', { profileName: selectedDns });
      notify(res, 'ok');
      await loadData();
    } catch (e) {
      notify(String(e), 'err');
    } finally {
      setLoading(false);
    }
  };

  return (
    <main className="min-h-screen bg-[#0a0a0a] text-[#e0e0e0] font-mono p-6 selection:bg-green-500/30">
      {/* Header */}
      <header className="max-w-2xl mx-auto mb-8 flex items-end justify-between border-b border-[#222] pb-4">
        <div>
          <h1 className="text-2xl font-bold tracking-tight text-green-400">VELOCITY</h1>
          <p className="text-xs text-[#555] mt-1">DPI BYPASS // DNS MANAGER</p>
        </div>
        <div className="flex items-center gap-3 text-xs">
          <span className={status?.zapret_running ? 'text-green-400' : 'text-[#444]'}>
            [{status?.zapret_running ? 'ACTIVE' : 'INACTIVE'}]
          </span>
          <span className={status?.is_admin ? 'text-green-400' : 'text-red-400'}>
            [{status?.is_admin ? 'ADMIN' : 'USER'}]
          </span>
        </div>
      </header>

      <div className="max-w-2xl mx-auto space-y-6">
        {/* Zapret Control */}
        <section className="border border-[#222] bg-[#0f0f0f] p-5">
          <div className="flex items-center justify-between mb-4">
            <h2 className="text-sm font-bold tracking-widest text-[#888]">ZAPRET MODE</h2>
            <button
              onClick={toggleZapret}
              disabled={loading || !status?.is_admin}
              className={`px-4 py-1.5 text-xs font-bold tracking-wider border transition-all ${
                status?.zapret_running
                  ? 'border-green-500/50 text-green-400 bg-green-500/10 hover:bg-green-500/20'
                  : 'border-[#333] text-[#666] hover:border-[#555] hover:text-[#999]'
              } disabled:opacity-30 disabled:cursor-not-allowed`}
            >
              {loading ? '...' : status?.zapret_running ? 'STOP' : 'START'}
            </button>
          </div>

          <div className="grid grid-cols-2 gap-2">
            {ZAPRET_MODES.map((m) => (
              <button
                key={m.id}
                onClick={() => setZapretMode(m.id)}
                disabled={status?.zapret_running}
                className={`text-left p-3 border transition-all ${
                  zapretMode === m.id
                    ? 'border-green-500/50 bg-green-500/5'
                    : 'border-[#1a1a1a] bg-[#0a0a0a] hover:border-[#333]'
                } ${status?.zapret_running ? 'opacity-40 cursor-not-allowed' : ''}`}
              >
                <div className={`text-xs font-bold tracking-wider ${
                  zapretMode === m.id ? 'text-green-400' : 'text-[#777]'
                }`}>
                  {m.label}
                </div>
                <div className="text-[10px] text-[#444] mt-1">{m.desc}</div>
              </button>
            ))}
          </div>
        </section>

        {/* DNS Control */}
        <section className="border border-[#222] bg-[#0f0f0f] p-5">
          <div className="flex items-center justify-between mb-4">
            <h2 className="text-sm font-bold tracking-widest text-[#888]">DNS SERVER</h2>
            <span className="text-[10px] text-[#444]">CURRENT: {status?.current_dns || '---'}</span>
          </div>

          <div className="grid grid-cols-3 gap-2 mb-4">
            {dnsProfiles.map((p) => (
              <button
                key={p.name}
                onClick={() => setSelectedDns(p.name)}
                className={`text-left p-3 border transition-all ${
                  selectedDns === p.name
                    ? 'border-green-500/50 bg-green-500/5'
                    : 'border-[#1a1a1a] bg-[#0a0a0a] hover:border-[#333]'
                }`}
              >
                <div className={`text-xs font-bold ${
                  selectedDns === p.name ? 'text-green-400' : 'text-[#777]'
                }`}>
                  {p.name}
                </div>
                <div className="text-[10px] text-[#444] mt-0.5 font-mono">{p.primary}</div>
              </button>
            ))}
          </div>

          <button
            onClick={applyDns}
            disabled={loading || !status?.is_admin}
            className="w-full py-2 text-xs font-bold tracking-widest border border-[#333] text-[#888] hover:border-green-500/50 hover:text-green-400 hover:bg-green-500/5 transition-all disabled:opacity-30 disabled:cursor-not-allowed"
          >
            {loading ? 'APPLYING...' : 'APPLY DNS'}
          </button>
        </section>

        {/* Status Bar */}
        <div className="border border-[#1a1a1a] bg-[#0a0a0a] px-4 py-2 flex justify-between text-[10px] text-[#444]">
          <span>SYS: {navigator.platform}</span>
          <span>TAURI v2 // REACT</span>
          <span>{new Date().toLocaleTimeString([], { hour12: false })}</span>
        </div>
      </div>

      {/* Toast */}
      {toast && (
        <div className={`fixed bottom-6 left-1/2 -translate-x-1/2 px-5 py-2 border text-xs font-bold tracking-wider ${
          toast.type === 'ok'
            ? 'border-green-500/30 bg-[#0a0a0a] text-green-400'
            : 'border-red-500/30 bg-[#0a0a0a] text-red-400'
        }`}>
          {toast.msg}
        </div>
      )}
    </main>
  );
}

export default App;