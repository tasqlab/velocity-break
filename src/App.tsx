import { useState, useEffect, useCallback, useRef, memo } from 'react';
import { invoke } from '@tauri-apps/api/core';

/* ──────────────────────────────────────────
   Types & Constants
────────────────────────────────────────── */
interface DnsProfile {
  name: string;
  primary: string;
  secondary: string;
  doh_url: string;
}

interface AppStatus {
  zapret_running: boolean;
  is_admin: boolean;
  current_dns: string;
  doh_active: boolean;
}

type ZapretMode = 'best' | 'all' | 'default';

const MODES: { id: ZapretMode; label: string; desc: string; icon: string }[] = [
  { id: 'best',    label: 'BEST',    desc: 'speed + compat',  icon: '' },
  { id: 'all',     label: 'ALL',     desc: 'max coverage',    icon: '◉' },
  { id: 'default', label: 'DEFAULT', desc: 'standard desync', icon: '≡' },
];

/* ──────────────────────────────────────────
   Sub-Components
────────────────────────────────────────── */

const WorldParallax = memo(() => {
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    let raf: number;
    let lastX = 0;
    let lastY = 0;

    const onMove = (e: MouseEvent) => {
      lastX = e.clientX;
      lastY = e.clientY;
      
      if (!raf) {
        raf = requestAnimationFrame(() => {
          if (!ref.current) return;
          const x = (lastX / window.innerWidth - 0.5) * 25;
          const y = (lastY / window.innerHeight - 0.5) * 25;
          ref.current.style.transform = `translate(${x}px, ${y}px)`;
          raf = 0;
        });
      }
    };

    window.addEventListener('mousemove', onMove, { passive: true });
    return () => { 
      window.removeEventListener('mousemove', onMove); 
      if (raf) cancelAnimationFrame(raf); 
    };
  }, []);

  // ... rest of component stays the same

  const dots: { cx: number; cy: number; r: number }[] = [];
  const continents = [
    [180,120],[200,110],[220,100],[240,95],[260,100],[280,110],[300,120],
    [190,140],[210,130],[230,125],[250,120],[270,125],[290,130],
    [200,160],[220,150],[240,145],[260,150],[280,155],
    [280,220],[290,240],[300,260],[310,280],[305,300],[295,320],
    [270,230],[275,250],[280,270],[285,290],
    [460,100],[470,90],[480,85],[490,90],[500,95],[510,100],
    [465,110],[475,105],[485,100],[495,105],[505,110],
    [470,160],[480,150],[490,145],[500,150],[510,160],[520,170],
    [475,180],[485,170],[495,165],[505,170],[515,180],
    [480,200],[490,190],[500,185],[510,190],[520,200],
    [540,80],[560,70],[580,65],[600,70],[620,75],[640,80],[660,85],
    [550,100],[570,90],[590,85],[610,90],[630,95],[650,100],
    [560,120],[580,110],[600,105],[620,110],[640,115],[660,120],
    [570,140],[590,130],[610,125],[630,130],[650,135],
    [680,260],[700,250],[720,245],[740,250],[760,260],
    [690,280],[710,270],[730,265],[750,270],
  ];
  continents.forEach(([x, y], i) => {
    dots.push({ cx: x, cy: y, r: i % 3 === 0 ? 2.5 : 1.5 });
  });

  return (
    <div ref={ref} className="fixed inset-0 z-0 pointer-events-none transition-transform duration-150 ease-out">
      <svg width="100%" height="100%" viewBox="0 0 960 480" style={{ opacity: 0.035 }}>
        {dots.filter((_, i) => i % 4 === 0).map((d, i) => {
          const next = dots[(i * 4 + 16) % dots.length];
          return <line key={`l${i}`} x1={d.cx} y1={d.cy} x2={next.cx} y2={next.cy}
            stroke="#4ade80" strokeWidth="0.3" opacity="0.4" />;
        })}
        {dots.map((d, i) => (
          <circle key={i} cx={d.cx} cy={d.cy} r={d.r} fill="#4ade80" />
        ))}
        <ellipse cx="480" cy="200" rx="320" ry="140" fill="none" stroke="#4ade80" strokeWidth="0.3" opacity="0.3" />
        <ellipse cx="480" cy="200" rx="380" ry="170" fill="none" stroke="#4ade80" strokeWidth="0.2" opacity="0.2" />
      </svg>
    </div>
  );
});

const StatusDot = memo(({ active, danger }: { active: boolean; danger?: boolean }) => (
  <span className={`inline-block w-1.5 h-1.5 rounded-full transition-all duration-500 ${
    active
      ? danger
        ? 'bg-red-400 status-dot-live shadow-[0_0_6px_rgba(248,113,113,0.8)]'
        : 'bg-green-400 status-dot-live shadow-[0_0_6px_rgba(74,222,128,0.8)]'
      : 'bg-[#333]'
  }`} />
));

const Badge = memo(({ active, label, danger }: { active: boolean; label: string; danger?: boolean }) => (
  <span className={`inline-flex items-center gap-1.5 text-[8px] font-bold tracking-widest px-2 py-1
    border rounded-sm transition-all duration-300 ${
    active
      ? danger ? 'border-red-500/25 text-red-400 bg-red-500/5'
               : 'border-green-500/25 text-green-400 bg-green-500/5'
      : 'border-[#1e1e1e] text-[#444]'
  }`}>
    <StatusDot active={active} danger={danger} />
    {label}
  </span>
));

/* ──────────────────────────────────────────
   Main App Component
────────────────────────────────────────── */
export default function App() {
  const [status, setStatus] = useState<AppStatus | null>(null);
  const [dnsProfiles, setDnsProfiles] = useState<DnsProfile[]>([]);
  const [selectedDns, setSelectedDns] = useState('Cloudflare');
  const [mode, setMode] = useState<ZapretMode>('best');
  const [loading, setLoading] = useState(false);
  const [toast, setToast] = useState<{ msg: string; ok: boolean } | null>(null);
  const [customOpen, setCustomOpen] = useState(false);
  const [customPrimary, setCustomPrimary] = useState('');
  const [customSecondary, setCustomSecondary] = useState('');
  const [customDoh, setCustomDoh] = useState('');
  const [dohEnabled, setDohEnabled] = useState(false);
  const [mounted, setMounted] = useState(false);
  const toastTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => { setMounted(true); }, []);

  useEffect(() => {
    invoke<DnsProfile[]>('get_dns_options').then(setDnsProfiles).catch(() => {});
  }, []);

  useEffect(() => {
    const poll = async () => {
    try {
      const s = await invoke<AppStatus>('check_status');
      setStatus(prev => {
        // Only trigger re-render if something actually changed
        if (
          prev?.zapret_running === s.zapret_running &&
          prev?.is_admin === s.is_admin &&
          prev?.current_dns === s.current_dns &&
          prev?.doh_active === s.doh_active
        ) {
          return prev; // Skip re-render entirely
        }
        return s;
      });
      setDohEnabled(s.doh_active);
    } catch {}
  };
  
  poll();
  const t = setInterval(poll, 5000);
  return () => clearInterval(t);
}, []);
  
  const notify = useCallback((msg: string, ok: boolean) => {
    if (toastTimer.current) clearTimeout(toastTimer.current);
    setToast({ msg, ok });
    toastTimer.current = setTimeout(() => setToast(null), 3500);
  }, []);

  const refreshStatus = useCallback(async () => {
    try { setStatus(await invoke<AppStatus>('check_status')); } catch {}
  }, []);

  const toggleZapret = useCallback(async () => {
  if (!status) return;
  setLoading(true);
  try {
    const res = await invoke<string>('toggle_zapret', { enable: !status.zapret_running, mode });
    notify(res, true);
    // Force immediate status refresh
    const newStatus = await invoke<AppStatus>('check_status');
    setStatus(newStatus);
  } catch (e) { 
    notify(String(e), false);
    // Refresh status even on error
    const s = await invoke<AppStatus>('check_status');
    setStatus(s);
  }
  finally { setLoading(false); }
}, [status, mode, notify]);

  const applyDns = useCallback(async (name: string) => {
    setLoading(true);
    try {
      const res = await invoke<string>('set_dns', { profileName: name, useDoh: dohEnabled });
      notify(res, true);
      await refreshStatus();
    } catch (e) { notify(String(e), false); }
    finally { setLoading(false); }
  }, [dohEnabled, notify, refreshStatus]);

  const applyCustomDns = useCallback(async () => {
    if (!customPrimary.trim()) { notify('Primary DNS required', false); return; }
    setLoading(true);
    try {
      const res = await invoke<string>('set_custom_dns', {
        primary: customPrimary.trim(),
        secondary: customSecondary.trim(),
        dohUrl: customDoh.trim(),
      });
      notify(res, true);
      await refreshStatus();
    } catch (e) { notify(String(e), false); }
    finally { setLoading(false); }
  }, [customPrimary, customSecondary, customDoh, notify, refreshStatus]);

  const resetDns = useCallback(async () => {
    setLoading(true);
    try {
      const res = await invoke<string>('reset_dns');
      notify(res, true);
      setDohEnabled(false);
      await refreshStatus();
    } catch (e) { notify(String(e), false); }
    finally { setLoading(false); }
  }, [notify, refreshStatus]);

  const minimizeToTray = () => invoke('hide_to_tray');
  const minimizeWin   = () => invoke('minimize_window');

  const running = status?.zapret_running ?? false;
  const admin   = status?.is_admin ?? false;

  return (
    <>
      {/* Background Layers (Fixed to viewport so they don't affect layout width) */}
      <div className="fixed inset-0 z-0 bg-mesh" />
      <div className="fixed inset-0 z-0 noise-overlay" />
      <WorldParallax />

      {/* Main App Container */}
      <div className={`relative z-10 flex flex-col h-full w-full bg-[#050505] transition-opacity duration-700 ${
        mounted ? 'opacity-100' : 'opacity-0'
      }`}>

        {/* Titlebar */}
        <div className="titlebar flex w-full items-center justify-between h-10 px-4 shrink-0 border-b border-[#1a1a1a] bg-[#050505]">
          <div className="flex items-center gap-3">
            <div className={`w-5 h-5 rounded border flex items-center justify-center transition-all duration-500 ${
              running ? 'border-green-500/40 bg-green-500/10' : 'border-[#2a2a2a] bg-[#0f0f0f]'
            }`}>
              <svg width="9" height="9" viewBox="0 0 24 24" fill="none"
                className={running ? 'text-green-400' : 'text-[#444]'}>
                <path d="M13 2L3 14h9l-1 8 10-12h-9l1-8z" stroke="currentColor" strokeWidth="3" strokeLinejoin="round"/>
              </svg>
            </div>
            <span className="text-[9px] font-bold tracking-[0.25em] text-[#666]">VELOCITY</span>
            <div className="flex gap-1.5 ml-1">
              <Badge active={running} label={running ? 'ON' : 'OFF'} />
              <Badge active={admin} label={admin ? 'ADMIN' : 'USER'} danger={!admin} />
              {dohEnabled && <Badge active={true} label="DoH" />}
            </div>
          </div>
          <div className="flex items-center gap-1">
            <button onClick={minimizeWin}
              className="w-8 h-6 flex items-center justify-center rounded text-[#555] hover:text-white hover:bg-white/5 transition-all duration-150">
              <svg width="10" height="10" viewBox="0 0 24 24"><path d="M5 12h14" stroke="currentColor" strokeWidth="2"/></svg>
            </button>
            <button onClick={minimizeToTray}
              className="w-8 h-6 flex items-center justify-center rounded text-[#555] hover:text-red-400 hover:bg-red-500/10 transition-all duration-150">
              <svg width="10" height="10" viewBox="0 0 24 24">
                <path d="M18 6L6 18M6 6l12 12" stroke="currentColor" strokeWidth="2" strokeLinecap="round"/>
              </svg>
            </button>
          </div>
        </div>

        {/* Scrollable Content Area (Flexbox Centering Magic) */}
        <div className="flex-1 w-full overflow-y-auto flex justify-center">
          
          {/* Centered Content Box */}
          <div className="w-full max-w-[360px] px-5 py-8 flex flex-col gap-6">

            {/* Toggle Section */}
            <section className="fade-up flex flex-col items-center gap-8 py-4">
              <div className="relative">
                {running && !loading && (
                  <>
                    <div className="pulse-ring" />
                    <div className="pulse-ring" style={{ animationDelay: '0.7s' }} />
                  </>
                )}
                {loading && (
                  <>
                    <div className="spinner-ring" />
                    <div className="spinner-ring-2" />
                  </>
                )}
                <button
                  onClick={toggleZapret}
                  disabled={loading || !admin}
                  className={`
                    relative w-36 h-36 rounded-full border-2
                    flex flex-col items-center justify-center gap-2
                    focus:outline-none select-none
                    transition-all duration-500 ease-out
                    ${running
                      ? 'border-green-500/60 bg-green-500/5 glow-active'
                      : 'border-[#222] bg-[#0a0a0a] hover:border-[#3a3a3a]'
                    }
                    ${loading ? 'scale-[0.96]' : 'active:scale-95'}
                    ${!admin ? 'opacity-40 cursor-not-allowed' : 'cursor-pointer'}
                  `}
                >
                  <div className={`absolute inset-3 rounded-full border transition-all duration-500 ${
                    running ? 'border-green-500/10' : 'border-[#1a1a1a]'
                  }`} />
                  <svg width="36" height="36" viewBox="0 0 24 24" fill="none"
                    className={`transition-all duration-500 ${
                      loading ? 'text-green-400/40'
                        : running ? 'text-green-400 drop-shadow-[0_0_8px_rgba(74,222,128,0.5)]'
                        : 'text-[#444]'
                    }`}>
                    <path d="M12 2v10" stroke="currentColor" strokeWidth="2" strokeLinecap="round"/>
                    <path d="M17.66 6.34a8 8 0 1 1-11.32 0" stroke="currentColor" strokeWidth="2" strokeLinecap="round"/>
                  </svg>
                  <span className={`text-sm font-bold tracking-[0.25em] transition-colors duration-500 ${
                    loading ? 'text-green-400/40' : running ? 'text-green-400' : 'text-[#555]'
                  }`}>
                    {loading ? '···' : running ? 'ON' : 'OFF'}
                  </span>
                  <span className="text-[8px] text-[#333] tracking-[0.3em] uppercase">Zapret Engine</span>
                </button>
              </div>

              {/* Mode Selector */}
              <div className="flex gap-3 w-full">
                {MODES.map((m) => (
                  <button key={m.id} onClick={() => setMode(m.id)} disabled={running}
                    className={`flex-1 py-3.5 px-2 rounded-lg border text-center
                      transition-all duration-200 btn-press ${
                      mode === m.id
                        ? 'border-green-500/40 bg-green-500/5 text-green-400'
                        : 'border-[#1a1a1a] bg-[#0a0a0a] text-[#555] hover:border-[#2e2e2e]'
                    } ${running ? 'opacity-40 cursor-not-allowed' : 'cursor-pointer'}`}>
                    <div className="text-lg mb-1">{m.icon}</div>
                    <div className="text-[10px] font-bold tracking-[0.15em]">{m.label}</div>
                    <div className="text-[8px] text-[#3a3a3a] mt-0.5">{m.desc}</div>
                  </button>
                ))}
              </div>
            </section>

            {/* DNS Section */}
            <section className="fade-up delay-2 glass rounded-xl p-6">
              <div className="flex justify-between items-center mb-5">
                <h2 className="text-[10px] font-bold tracking-[0.2em] text-[#555] uppercase flex items-center gap-2">
                  <span className="w-1.5 h-1.5 bg-green-500/60 rounded-full" />
                  DNS SERVER
                </h2>
                <div className="flex items-center gap-3">
                  <span className="text-[9px] text-[#333] font-mono">{status?.current_dns ?? '---'}</span>
                  <button
                    onClick={() => setDohEnabled(!dohEnabled)}
                    className={`flex items-center gap-1.5 text-[8px] font-bold tracking-widest px-2.5 py-1 rounded border transition-all duration-200 ${
                      dohEnabled
                        ? 'border-green-500/30 text-green-400 bg-green-500/5'
                        : 'border-[#1e1e1e] text-[#444] hover:border-[#333]'
                    }`}
                  >
                    <StatusDot active={dohEnabled} />
                    DoH
                  </button>
                </div>
              </div>

              {dohEnabled && (
                <div className="fade-up mb-5 px-3.5 py-2.5 rounded-lg border border-green-500/15 bg-green-500/5
                  text-[9px] text-green-400/70 tracking-wider flex items-center gap-2.5">
                  <svg width="11" height="11" viewBox="0 0 24 24" fill="none" className="shrink-0">
                    <rect x="3" y="11" width="18" height="11" rx="2" stroke="currentColor" strokeWidth="2"/>
                    <path d="M7 11V7a5 5 0 0110 0v4" stroke="currentColor" strokeWidth="2"/>
                  </svg>
                  DNS queries encrypted via HTTPS tunnel
                </div>
              )}

              <div className="grid grid-cols-2 gap-2 mb-4">
                {dnsProfiles.map(p => (
                  <button key={p.name}
                    onClick={() => { setSelectedDns(p.name); setCustomOpen(false); }}
                    disabled={loading}
                    className={`glass-hover rounded-lg p-3.5 text-left border transition-all duration-200 btn-press ${
                      selectedDns === p.name && !customOpen
                        ? 'border-green-500/40 bg-green-500/5'
                        : 'border-[#1a1a1a] bg-[#080808]'
                    } ${loading ? 'opacity-50 cursor-not-allowed' : 'cursor-pointer'}`}>
                    <div className="flex items-center gap-1.5 mb-1.5">
                      <StatusDot active={selectedDns === p.name && !customOpen} />
                      <span className={`text-[11px] font-bold transition-colors ${
                        selectedDns === p.name && !customOpen ? 'text-green-400' : 'text-[#777]'
                      }`}>{p.name}</span>
                    </div>
                    <div className="text-[9px] text-[#3a3a3a] font-mono">{p.primary}</div>
                    {dohEnabled && (
                      <div className="text-[7px] text-green-500/40 mt-1.5 flex items-center gap-1">
                        <svg width="6" height="6" viewBox="0 0 24 24" fill="none">
                          <rect x="3" y="11" width="18" height="11" rx="2" stroke="currentColor" strokeWidth="2"/>
                          <path d="M7 11V7a5 5 0 0110 0v4" stroke="currentColor" strokeWidth="2"/>
                        </svg>
                        DoH
                      </div>
                    )}
                  </button>
                ))}

                <button onClick={() => setCustomOpen(true)} disabled={loading}
                  className={`rounded-lg p-3.5 text-left border-2 border-dashed transition-all duration-200 btn-press ${
                    customOpen ? 'border-green-500/40 bg-green-500/5'
                      : 'border-[#222] hover:border-[#3a3a3a] hover:bg-[#0a0a0a]'
                  } ${loading ? 'opacity-50 cursor-not-allowed' : 'cursor-pointer'}`}>
                  <div className="flex items-center gap-1.5 mb-1.5">
                    <StatusDot active={customOpen} />
                    <span className={`text-[11px] font-bold ${customOpen ? 'text-green-400' : 'text-[#555]'}`}>CUSTOM</span>
                  </div>
                  <div className="text-[9px] text-[#3a3a3a]">manual IP</div>
                </button>
              </div>

              {customOpen && (
                <div className="fade-up mb-5 rounded-lg border border-[#1a1a1a] bg-[#070707] p-5">
                  <div className="grid grid-cols-2 gap-4 mb-4">
                    <div>
                      <label className="block text-[7px] text-[#444] tracking-[0.2em] mb-2 uppercase">
                        Primary DNS <span className="text-green-500">*</span>
                      </label>
                      <input type="text" value={customPrimary}
                        onChange={e => setCustomPrimary(e.target.value)} placeholder="1.1.1.1"
                        className="w-full bg-[#0e0e0e] border border-[#222] rounded-md text-[#d4d4d4] text-xs px-3.5 py-2.5 outline-none focus:border-green-500/40 placeholder:text-[#2a2a2a] transition-all duration-200" />
                    </div>
                    <div>
                      <label className="block text-[7px] text-[#444] tracking-[0.2em] mb-2 uppercase">Secondary DNS</label>
                      <input type="text" value={customSecondary}
                        onChange={e => setCustomSecondary(e.target.value)} placeholder="1.0.0.1"
                        className="w-full bg-[#0e0e0e] border border-[#222] rounded-md text-[#d4d4d4] text-xs px-3.5 py-2.5 outline-none focus:border-green-500/40 placeholder:text-[#2a2a2a] transition-all duration-200" />
                    </div>
                  </div>
                  {dohEnabled && (
                    <div className="mb-4">
                      <label className="block text-[7px] text-[#444] tracking-[0.2em] mb-2 uppercase">DoH Template URL</label>
                      <input type="text" value={customDoh}
                        onChange={e => setCustomDoh(e.target.value)} placeholder="https://dns.example.com/dns-query"
                        className="w-full bg-[#0e0e0e] border border-[#222] rounded-md text-[#d4d4d4] text-xs px-3.5 py-2.5 outline-none focus:border-green-500/40 placeholder:text-[#2a2a2a] transition-all duration-200" />
                    </div>
                  )}
                  <button onClick={applyCustomDns}
                    disabled={loading || !admin || !customPrimary.trim()}
                    className="btn-press w-full py-3 rounded-md text-[10px] font-bold tracking-[0.2em]
                      border border-green-500/40 text-green-400 hover:bg-green-500/10
                      transition-all duration-200 disabled:opacity-30 disabled:cursor-not-allowed">
                    {loading ? 'APPLYING...' : 'APPLY CUSTOM DNS'}
                  </button>
                </div>
              )}

              {!customOpen && (
                <button onClick={() => applyDns(selectedDns)}
                  disabled={loading || !admin}
                  className="btn-press w-full py-3 rounded-md text-[10px] font-bold tracking-[0.2em]
                    border border-green-500/40 text-green-400 hover:bg-green-500/10
                    transition-all duration-200 disabled:opacity-30 disabled:cursor-not-allowed">
                  {loading ? 'APPLYING...' : `APPLY ${selectedDns.toUpperCase()}${dohEnabled ? ' + DoH' : ''}`}
                </button>
              )}

              <div className="mt-3">
                <button onClick={resetDns}
                  disabled={loading || !admin}
                  className="btn-press w-full py-2.5 rounded-md text-[9px] font-bold tracking-[0.2em]
                    border border-[#1a1a1a] text-[#444] hover:border-red-500/30 hover:text-red-400 hover:bg-red-500/5
                    transition-all duration-200 disabled:opacity-30 disabled:cursor-not-allowed">
                  RESET TO DHCP
                </button>
              </div>
            </section>

            {/* Admin Warning */}
            {!admin && (
              <div className="fade-up delay-3 rounded-lg border border-red-500/20 bg-red-500/5 px-4 py-3.5 flex items-center gap-3">
                <span className="text-red-400 text-sm">⚠</span>
                <p className="text-[9px] text-red-400/80 tracking-[0.1em]">RUN AS ADMINISTRATOR TO ENABLE ALL FEATURES</p>
              </div>
            )}
            {/* Auto-start Toggle */}
<section className="fade-up delay-4 glass rounded-lg p-4 flex items-center justify-between">
  <div className="flex items-center gap-3">
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" className="text-[#555]">
      <path d="M12 2v4M12 18v4M4.93 4.93l2.83 2.83M16.24 16.24l2.83 2.83M2 12h4M18 12h4M4.93 19.07l2.83-2.83M16.24 7.76l2.83-2.83" stroke="currentColor" strokeWidth="2" strokeLinecap="round"/>
    </svg>
    <span className="text-[10px] font-bold tracking-[0.15em] text-[#666]">START WITH WINDOWS</span>
  </div>
  <button
    onClick={async () => {
      try {
        await invoke('set_autostart', { enable: true }); // Simplified for now
        notify('Auto-start enabled', true);
      } catch (e) { notify(String(e), false); }
    }}
    className="text-[9px] font-bold tracking-widest px-3 py-1.5 rounded border border-[#222] text-[#555] hover:border-green-500/40 hover:text-green-400 transition-all"
  >
    ENABLE
  </button>
</section>
            {/* Footer */}
            <footer className="pb-4 pt-2">
              <div className="h-px bg-gradient-to-r from-transparent via-[#1e1e1e] to-transparent mb-3" />
              <div className="flex justify-between text-[7px] text-[#2a2a2a] tracking-[0.2em]">
                <span>TAURI v2 // REACT // RUST</span>
                <span>{new Date().toLocaleDateString('en-US', { month:'short', day:'2-digit', year:'numeric' })}</span>
              </div>
            </footer>
          </div>
        </div>
      </div>

      {/* Toast Notification */}
      {toast && (
        <div className={`toast-anim fixed bottom-6 left-1/2 z-50 flex items-center gap-2.5 px-5 py-3 rounded-lg
          border backdrop-blur-xl text-[10px] font-bold tracking-[0.1em] ${
          toast.ok
            ? 'border-green-500/25 bg-[#050505]/90 text-green-400'
            : 'border-red-500/25 bg-[#050505]/90 text-red-400'
        }`}>
          <StatusDot active={true} danger={!toast.ok} />
          {toast.msg}
        </div>
      )}
    </>
  );
}