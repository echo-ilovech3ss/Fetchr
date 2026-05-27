import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { 
  Download, 
  Play, 
  Settings, 
  Activity, 
  Trash2, 
  FolderOpen, 
  RefreshCw, 
  CheckCircle, 
  Search, 
  Layers, 
  Sliders, 
  ToggleLeft,
  ToggleRight,
  Cpu,
  AlertTriangle
} from 'lucide-react';

// ==========================================
// Types & Models
// ==========================================

interface Task {
  id: string;
  task_type: {
    type: string;
    format_preset?: string;
    audio_format?: string;
  };
  url: string;
  status: 'Pending' | 'Downloading' | 'Paused' | 'Completed' | 'Failed' | 'Interrupted';
  progress: number;
  speed: string | null;
  eta: string | null;
  file_path: string | null;
  error_msg: string | null;
  retry_count: number;
  created_at: string;
}

interface HistoryItem {
  id: string;
  title: string;
  url: string;
  file_path: string;
  file_size: number;
  duration: number;
  thumbnail_path: string | null;
  resolution: string | null;
  source_site: string | null;
  download_duration_secs: number;
  completed_at: string;
}



interface SelfCheckStatus {
  yt_dlp: {
    status: 'OK' | 'MISSING' | 'ERROR';
    path?: string;
    version?: string;
    error?: string;
  };
  ffmpeg: {
    status: 'OK' | 'MISSING';
    path?: string;
    error?: string;
  };
  database: 'OK' | 'CORRUPT';
  bin_dir: string;
}

export default function App() {
  // Navigation & UI States
  const [activeTab, setActiveTab] = useState<'downloader' | 'queue' | 'history' | 'settings'>('downloader');
  const [advancedMode, setAdvancedMode] = useState<boolean>(false);
  const [toast, setToast] = useState<{ message: string; type: 'success' | 'info' | 'error' } | null>(null);

  // Onboarding Setup Wizard State
  const [isOnboarding, setIsOnboarding] = useState<boolean>(false);
  const [setupStep, setSetupStep] = useState<'welcome' | 'downloading' | 'completed'>('welcome');

  // Downloader Inputs
  const [urlInput, setUrlInput] = useState('');
  const [isAnalyzing, setIsAnalyzing] = useState(false);
  const [analyzedMedia, setAnalyzedMedia] = useState<{
    metadata: {
      title: string;
      description: string | null;
      duration: number;
      uploader: string | null;
      uploader_url: string | null;
      thumbnail_url: string | null;
      webpage_url: string;
      formats: Array<{ format_id: string; ext: string; resolution: string | null; filesize: number | null; vcodec?: string | null; acodec?: string | null }>;
      extractor: string;
    };
    capabilities: {
      platform_name: string;
      supports_audio: boolean;
      supports_subtitles: boolean;
      supports_playlists: boolean;
      supports_login: boolean;
    };
  } | null>(null);

  // Dynamic Format / Quality Picker states
  const [downloadType, setDownloadType] = useState<'video' | 'audio'>('video');
  const [videoFormat, setVideoFormat] = useState<'mp4' | 'mkv'>('mp4');
  const [videoQuality, setVideoQuality] = useState<string>('best');
  const [audioFormat, setAudioFormat] = useState<'mp3' | 'wav'>('mp3');

  // Queue & History States
  const [queue, setQueue] = useState<Task[]>([]);
  const [history, setHistory] = useState<HistoryItem[]>([]);
  const [searchQuery, setSearchQuery] = useState('');

  // Settings State
  const [settings, setSettings] = useState({
    download_directory: '',
    max_concurrent_tasks: 2,
    skip_previously_downloaded: false,
    cookies_browser: '',
    yt_dlp_channel: 'Stable',
    advanced_mode: false,
    custom_yt_dlp_flags: '',
    custom_yt_dlp_path: '',
  });

  // Diagnostics & Repair States
  const [selfCheck, setSelfCheck] = useState<SelfCheckStatus | null>(null);
  const [isRepairing, setIsRepairing] = useState(false);

  // ==========================================
  // Core Startup Checks & Handlers
  // ==========================================

  useEffect(() => {
    // 1. Run system diagnostics self-check on launch
    triggerSelfCheck();

    // 2. Fetch basic system configurations
    fetchSettings();

    // 3. Fetch Initial Data
    fetchQueue();
    fetchHistory();

    // 4. Subscribe to Real-time Progress IPC event streams
    const unlistenPromise = listen<Task>('task-update', (event) => {
      setQueue((prevQueue) => {
        const updatedTask = event.payload;
        const exists = prevQueue.some((t) => t.id === updatedTask.id);
        if (exists) {
          return prevQueue.map((t) => (t.id === updatedTask.id ? updatedTask : t));
        } else {
          return [...prevQueue, updatedTask];
        }
      });
    });

    const unlistenCompletePromise = listen('queue-complete', () => {
      showToast('All downloads completed!', 'success');
      fetchHistory(); // Refresh completed history logs
    });

    return () => {
      unlistenPromise.then((fn) => fn());
      unlistenCompletePromise.then((fn) => fn());
    };
  }, []);

  // Poll queue state slightly as fallback safety
  useEffect(() => {
    const timer = setInterval(() => {
      fetchQueue();
    }, 3000);
    return () => clearInterval(timer);
  }, []);

  // Clipboard Watcher Feature
  useEffect(() => {
    const handleFocus = async () => {
      try {
        const text = await navigator.clipboard.readText();
        const trimmed = text.trim();
        const isValid = trimmed.startsWith('http://') || trimmed.startsWith('https://');
        
        const isSupported = trimmed.includes('youtube.com') || 
                            trimmed.includes('youtu.be') || 
                            trimmed.includes('instagram.com') ||
                            trimmed.includes('facebook.com') ||
                            trimmed.includes('tiktok.com');

        if (isValid && isSupported && trimmed !== urlInput && !analyzedMedia && activeTab === 'downloader' && !isOnboarding) {
          setUrlInput(trimmed);
          showToast('Media link detected from your clipboard!', 'info');
        }
      } catch (e) {
        // Clipboard read permission might not be active, fail silently
      }
    };

    window.addEventListener('focus', handleFocus);
    return () => window.removeEventListener('focus', handleFocus);
  }, [urlInput, analyzedMedia, activeTab, isOnboarding]);

  const showToast = (message: string, type: 'success' | 'info' | 'error' = 'info') => {
    setToast({ message, type });
    setTimeout(() => setToast(null), 4000);
  };

  // ==========================================
  // IPC Invocation Gateways
  // ==========================================

  const fetchSettings = async () => {
    try {
      const res: any = await invoke('get_settings');
      setSettings(res);
      setAdvancedMode(res.advanced_mode);
    } catch (e) {
      showToast('Failed to load settings: ' + e, 'error');
    }
  };

  const fetchQueue = async () => {
    try {
      const res: any = await invoke('get_queue');
      setQueue(res);
    } catch (e) {
      // Fail silently
    }
  };

  const fetchHistory = async () => {
    try {
      const res: any = await invoke('get_history', { search: searchQuery || null });
      setHistory(res);
    } catch (e) {
      showToast('Failed to load history: ' + e, 'error');
    }
  };

  const triggerSelfCheck = async () => {
    try {
      const res: any = await invoke('run_self_check');
      setSelfCheck(res);
      
      // If yt-dlp is missing, launch friendly Onboarding setup
      if (res.yt_dlp.status !== 'OK') {
        setIsOnboarding(true);
        setSetupStep('welcome');
      }
    } catch (e) {
      showToast('Diagnostics failed: ' + e, 'error');
    }
  };

  const handleStartOnboardingSetup = async () => {
    setSetupStep('downloading');
    try {
      await invoke('force_yt_dlp_update');
      setSetupStep('completed');
      
      // Dynamic refresh
      const check: any = await invoke('run_self_check');
      setSelfCheck(check);
      
      setTimeout(() => {
        setIsOnboarding(false);
        showToast('System is ready! Welcome to Fetchr.', 'success');
      }, 1500);
    } catch (e) {
      showToast('Setup failed: ' + e, 'error');
      setSetupStep('welcome');
    }
  };

  const handleRepairBinaries = async () => {
    setIsRepairing(true);
    showToast('Updating download engine in the background...', 'info');
    try {
      const res: string = await invoke('force_yt_dlp_update');
      showToast(res, 'success');
      triggerSelfCheck();
    } catch (e) {
      showToast('Repair failed: ' + e, 'error');
    } finally {
      setIsRepairing(false);
    }
  };

  const handleSaveSettings = async (newSettings: any) => {
    try {
      await invoke('save_settings', { settings: newSettings });
      setSettings(newSettings);
      setAdvancedMode(newSettings.advanced_mode);
      showToast('Settings saved successfully.', 'success');
    } catch (e) {
      showToast('Failed to save settings: ' + e, 'error');
    }
  };

  // ==========================================
  // Downloader Mechanics
  // ==========================================

  const handleAnalyze = async () => {
    if (!urlInput.trim()) {
      showToast('Please paste a link first.', 'error');
      return;
    }

    setIsAnalyzing(true);
    setAnalyzedMedia(null);
    showToast('Scanning link details...', 'info');

    try {
      const res: any = await invoke('analyze_url', {
        url: urlInput.trim(),
        cookiesBrowser: settings.cookies_browser || null
      });
      setAnalyzedMedia(res);
      setVideoQuality('best'); // Always default to best quality for each new URL
      showToast('Details loaded!', 'success');
    } catch (e) {
      showToast('Could not load link details. Check settings or connection.', 'error');
    } finally {
      setIsAnalyzing(false);
    }
  };

  const handleStartDownload = async () => {
    if (!analyzedMedia) return;

    try {
      let taskType;
      if (downloadType === 'audio') {
        taskType = {
          type: 'DownloadAudio',
          audio_format: audioFormat
        };
      } else {
        taskType = {
          type: 'DownloadVideo',
          format_preset: `dynamic:${videoQuality}:${videoFormat}`
        };
      }

      await invoke('add_download_task', {
        url: urlInput.trim(),
        taskType
      });

      showToast('Download started. Track it in "Active Downloads".', 'success');
      setAnalyzedMedia(null);
      setUrlInput('');
      setActiveTab('queue');
      fetchQueue();
    } catch (e) {
      showToast('Failed to start download: ' + e, 'error');
    }
  };

  // ==========================================
  // Queue Controls
  // ==========================================

  const handleCancelDownload = async (id: string) => {
    try {
      await invoke('cancel_download', { id });
      showToast('Download paused.', 'info');
      fetchQueue();
    } catch (e) {
      showToast('Failed to pause: ' + e, 'error');
    }
  };

  const handleResumeDownload = async (id: string) => {
    try {
      await invoke('resume_download', { id });
      showToast('Resuming download.', 'success');
      fetchQueue();
    } catch (e) {
      showToast('Failed to resume: ' + e, 'error');
    }
  };

  const handleDeleteTask = async (id: string) => {
    try {
      await invoke('delete_task', { id });
      showToast('Download task removed.', 'info');
      fetchQueue();
    } catch (e) {
      showToast('Failed to remove task: ' + e, 'error');
    }
  };

  const handleClearQueue = async () => {
    try {
      await invoke('clear_queue');
      showToast('Completed downloads cleared from queue.', 'info');
      fetchQueue();
    } catch (e) {
      showToast('Failed to clear queue: ' + e, 'error');
    }
  };

  // ==========================================
  // History Controls
  // ==========================================

  const handleDeleteHistoryItem = async (id: string) => {
    try {
      await invoke('delete_history_item', { id });
      showToast('Item removed from library.', 'info');
      fetchHistory();
    } catch (e) {
      showToast('Failed to remove history: ' + e, 'error');
    }
  };

  const handleClearHistory = async () => {
    if (confirm('Are you sure you want to clear your complete history list?')) {
      try {
        await invoke('clear_history');
        showToast('Library history cleared.', 'info');
        fetchHistory();
      } catch (e) {
        showToast('Failed to clear history: ' + e, 'error');
      }
    }
  };

  const handleOpenFolder = async (filePath: string) => {
    try {
      await invoke('locate_file', { path: filePath });
      showToast('Revealed file in folder.', 'success');
    } catch (e) {
      showToast('Failed to reveal file: ' + e, 'error');
    }
  };

  // Helpers
  const formatBytes = (bytes: number) => {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
  };

  const formatDuration = (seconds: number) => {
    const h = Math.floor(seconds / 3600);
    const m = Math.floor((seconds % 3600) / 60);
    const s = Math.floor(seconds % 60);
    if (h > 0) {
      return `${h}:${m.toString().padStart(2, '0')}:${s.toString().padStart(2, '0')}`;
    }
    return `${m}:${s.toString().padStart(2, '0')}`;
  };

  const getDetectedQualities = () => {
    if (!analyzedMedia) return [];
    
    // Unique qualities maps standardized smaller dimension to actual larger dimension
    const uniqueQualities = new Map<number, number>(); // Map<stdHeight, actualHeight>
    
    analyzedMedia.metadata.formats.forEach((f) => {
      const isVideo = f.vcodec && f.vcodec !== 'none';
      if (isVideo && f.resolution) {
        const parts = f.resolution.split('x');
        if (parts.length === 2) {
          const w = parseInt(parts[0], 10);
          const h = parseInt(parts[1], 10);
          if (!isNaN(w) && !isNaN(h) && w > 0 && h > 0) {
            const stdHeight = Math.min(w, h); // e.g. Math.min(1080, 1920) -> 1080
            const actualHeight = h; // e.g. 1920
            
            const existing = uniqueQualities.get(stdHeight);
            if (!existing || actualHeight > existing) {
              uniqueQualities.set(stdHeight, actualHeight);
            }
          }
        }
      }
    });

    if (uniqueQualities.size === 0) {
      return [{ value: 'best', label: 'Best Available Quality' }];
    }

    // Sort standardized heights descending (e.g. 2160, 1440, 1080, 720)
    const sortedStdHeights = Array.from(uniqueQualities.keys()).sort((a, b) => b - a);
    
    const result = [{ value: 'best', label: 'Best Available Quality' }];
    
    sortedStdHeights.forEach((std) => {
      const actual = uniqueQualities.get(std)!;
      let label = '';
      if (std >= 2160) label = `${std}p (4K Ultra HD)`;
      else if (std >= 1440) label = `${std}p (2K Quad HD)`;
      else if (std >= 1080) label = `${std}p (Full HD)`;
      else if (std >= 720) label = `${std}p (HD)`;
      else if (std >= 480) label = `${std}p (Standard Definition)`;
      else label = `${std}p (Low Quality)`;
      
      result.push({
        value: `${actual}p`, // The value is the actual height (e.g. 1920p)
        label: label
      });
    });
    
    return result;
  };

  // ==========================================
  // Onboarding Setup Wizard Screen
  // ==========================================

  if (isOnboarding) {
    return (
      <div style={{
        width: '100vw',
        height: '100vh',
        background: '#090c15',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        padding: '2rem',
        backgroundImage: 'radial-gradient(circle at top, rgba(99, 102, 241, 0.05), transparent 70%)'
      }}>
        <div className="cyber-card" style={{ maxWidth: '520px', width: '100%', textAlign: 'center', padding: '3rem 2.5rem' }}>
          
          <div style={{ display: 'flex', justifyContent: 'center', marginBottom: '1.5rem' }}>
            <div style={{
              width: '72px',
              height: '72px',
              borderRadius: '24px',
              background: 'rgba(99, 102, 241, 0.08)',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              border: '1px solid rgba(99, 102, 241, 0.15)'
            }}>
              <Download size={32} color="var(--accent-indigo)" />
            </div>
          </div>

          <h2 style={{ fontSize: '1.6rem', fontWeight: 800, marginBottom: '0.75rem', letterSpacing: '-0.02em' }}>
            Welcome to Fetchr
          </h2>

          {setupStep === 'welcome' && (
            <div>
              <p style={{ color: 'var(--text-sub)', fontSize: '0.9rem', lineHeight: '1.6', marginBottom: '2rem' }}>
                Let's set up your download engine. This is fully automatic, takes about 30 seconds, and ensures you can download videos at maximum speed and quality.
              </p>
              <button className="btn-primary" style={{ width: '100%' }} onClick={handleStartOnboardingSetup}>
                Start Automatic Setup
              </button>
            </div>
          )}

          {setupStep === 'downloading' && (
            <div>
              <p style={{ color: 'var(--text-sub)', fontSize: '0.9rem', lineHeight: '1.6', marginBottom: '2rem' }}>
                Downloading the latest safe media extraction components in the background. Please keep Fetchr open...
              </p>
              <div style={{ display: 'flex', flexDirection: 'column', gap: '0.5rem' }}>
                <div className="progress-bar-container">
                  <div className="progress-bar-fill" style={{ width: '65%', animation: 'shimmer 1.5s infinite' }}></div>
                </div>
                <span className="mono-metric" style={{ fontSize: '0.75rem', color: 'var(--accent-indigo)' }}>
                  Downloading core assets...
                </span>
              </div>
            </div>
          )}

          {setupStep === 'completed' && (
            <div>
              <div style={{ display: 'flex', justifyContent: 'center', gap: '0.5rem', alignItems: 'center', color: 'var(--accent-green)', fontWeight: 700, fontSize: '0.95rem', marginBottom: '1.5rem' }}>
                <CheckCircle size={20} />
                Setup Complete!
              </div>
              <p style={{ color: 'var(--text-sub)', fontSize: '0.9rem', lineHeight: '1.6' }}>
                Launching your clean media library...
              </p>
            </div>
          )}

        </div>
      </div>
    );
  }

  // ==========================================
  // Main Panel Shell
  // ==========================================

  return (
    <div style={{ display: 'flex', width: '100%', height: '100%', position: 'relative' }}>
      
      {/* Sidebar Navigation */}
      <aside style={{
        width: '260px',
        background: '#090c15',
        borderRight: '1px solid var(--border-slate)',
        display: 'flex',
        flexDirection: 'column',
        padding: '1.75rem 1.25rem',
        justifyContent: 'space-between'
      }}>
        <div>
          {/* Brand Logo */}
          <div style={{ display: 'flex', alignItems: 'center', gap: '0.75rem', marginBottom: '2.5rem', paddingLeft: '0.5rem' }}>
            <Download size={26} color="var(--accent-indigo)" />
            <h1 style={{ fontSize: '1.25rem', fontWeight: 800, letterSpacing: '-0.02em', color: '#fff' }}>
              Fetchr
            </h1>
          </div>

          {/* Navigation Links */}
          <nav style={{ display: 'flex', flexDirection: 'column', gap: '0.5rem' }}>
            <div className={`nav-tab ${activeTab === 'downloader' ? 'active' : ''}`} onClick={() => setActiveTab('downloader')}>
              <Download size={20} />
              <span style={{ fontSize: '0.9rem' }}>Download Media</span>
            </div>
            
            <div className={`nav-tab ${activeTab === 'queue' ? 'active' : ''}`} onClick={() => setActiveTab('queue')}>
              <Activity size={20} />
              <span style={{ fontSize: '0.9rem', display: 'flex', width: '100%', justifyContent: 'space-between', alignItems: 'center' }}>
                Active Downloads
                {queue.filter(t => t.status === 'Downloading' || t.status === 'Pending').length > 0 && (
                  <span className="mono-metric" style={{ background: '#fff', color: '#000', fontSize: '0.7rem', padding: '2px 6px', borderRadius: '6px', fontWeight: 700 }}>
                    {queue.filter(t => t.status === 'Downloading' || t.status === 'Pending').length}
                  </span>
                )}
              </span>
            </div>
            
            <div className={`nav-tab ${activeTab === 'history' ? 'active' : ''}`} onClick={() => setActiveTab('history')}>
              <Layers size={20} />
              <span style={{ fontSize: '0.9rem' }}>Saved Library</span>
            </div>
            
            <div className={`nav-tab ${activeTab === 'settings' ? 'active' : ''}`} onClick={() => setActiveTab('settings')}>
              <Settings size={20} />
              <span style={{ fontSize: '0.9rem' }}>App Settings</span>
            </div>
          </nav>
        </div>

        {/* Binary Engine Diagnostics Status Bar */}
        {selfCheck && (
          <div className="cyber-card" style={{ padding: '0.85rem', fontSize: '0.75rem', display: 'flex', flexDirection: 'column', gap: '0.4rem', borderTop: 'none' }}>
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
              <span style={{ color: 'var(--text-sub)' }}>System core:</span>
              <span style={{ 
                color: selfCheck.yt_dlp.status === 'OK' ? 'var(--accent-green)' : 'var(--accent-rose)',
                fontWeight: 700,
                display: 'flex',
                alignItems: 'center',
                gap: '4px'
              }}>
                <span style={{ width: '6px', height: '6px', borderRadius: '50%', background: selfCheck.yt_dlp.status === 'OK' ? 'var(--accent-green)' : 'var(--accent-rose)', display: 'inline-block' }}></span>
                {selfCheck.yt_dlp.status === 'OK' ? 'Ready' : 'Setup Required'}
              </span>
            </div>
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
              <span style={{ color: 'var(--text-sub)' }}>Video merger:</span>
              <span style={{ 
                color: selfCheck.ffmpeg.status === 'OK' ? 'var(--accent-green)' : 'var(--accent-rose)',
                fontWeight: 700,
                display: 'flex',
                alignItems: 'center',
                gap: '4px'
              }}>
                <span style={{ width: '6px', height: '6px', borderRadius: '50%', background: selfCheck.ffmpeg.status === 'OK' ? 'var(--accent-green)' : 'var(--accent-rose)', display: 'inline-block' }}></span>
                {selfCheck.ffmpeg.status === 'OK' ? 'Active' : 'Missing'}
              </span>
            </div>
          </div>
        )}
      </aside>

      {/* Main Panel Shell */}
      <main style={{
        flex: 1,
        background: 'radial-gradient(circle at top right, rgba(99, 102, 241, 0.02), transparent 60%), #0b0f19',
        padding: '2.5rem 3rem',
        display: 'flex',
        flexDirection: 'column',
        height: '100vh',
        overflowY: 'auto'
      }}>

        {/* 1. Downloader Tab */}
        {activeTab === 'downloader' && (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '1.75rem', flex: 1 }}>
            
            {/* Input Bar */}
            <div style={{ display: 'flex', flexDirection: 'column', gap: '0.6rem' }}>
              <h2 style={{ fontSize: '1.25rem', fontWeight: 700, letterSpacing: '-0.01em' }}>PASTE VIDEO OR AUDIO LINK</h2>
              <div style={{ display: 'flex', gap: '0.75rem' }}>
                <input 
                  type="text" 
                  className="cyber-input" 
                  style={{ flex: 1 }} 
                  placeholder="Paste any YouTube video link, Instagram reel, or other media page..." 
                  value={urlInput}
                  onChange={(e) => setUrlInput(e.target.value)}
                  onKeyDown={(e) => e.key === 'Enter' && handleAnalyze()}
                />
                <button className="btn-primary" onClick={handleAnalyze} disabled={isAnalyzing}>
                  <RefreshCw size={18} className={isAnalyzing ? 'spin-anim' : ''} />
                  {isAnalyzing ? 'Scanning...' : 'Scan Link'}
                </button>
              </div>
            </div>

            {/* Media Info Extraction Card */}
            {analyzedMedia && (
              <div className="cyber-card" style={{ display: 'flex', gap: '1.75rem', flex: 1, maxHeight: '420px', minHeight: '340px' }}>
                
                {/* Left Side: Thumbnail Preview */}
                <div style={{ width: '40%', height: '100%', position: 'relative', borderRadius: '12px', overflow: 'hidden', border: '1px solid var(--border-slate)' }}>
                  {analyzedMedia.metadata.thumbnail_url ? (
                    <img 
                      src={analyzedMedia.metadata.thumbnail_url} 
                      alt="Thumbnail Preview" 
                      style={{ width: '100%', height: '100%', objectFit: 'cover' }} 
                    />
                  ) : (
                    <div style={{ display: 'flex', width: '100%', height: '100%', background: '#090b11', alignItems: 'center', justifyContent: 'center' }}>
                      <Download size={48} color="rgba(255, 255, 255, 0.05)" />
                    </div>
                  )}
                  <div style={{
                    position: 'absolute',
                    bottom: '0.75rem',
                    right: '0.75rem',
                    background: 'rgba(0, 0, 0, 0.8)',
                    padding: '3px 8px',
                    borderRadius: '6px',
                    fontSize: '0.75rem',
                    fontWeight: 600
                  }} className="mono-metric">
                    {formatDuration(analyzedMedia.metadata.duration)}
                  </div>
                  
                  {/* Extractor Badge */}
                  <div style={{
                    position: 'absolute',
                    top: '0.75rem',
                    left: '0.75rem',
                    background: 'rgba(99, 102, 241, 0.95)',
                    padding: '3px 10px',
                    borderRadius: '6px',
                    fontSize: '0.7rem',
                    fontWeight: 700,
                    letterSpacing: '0.05em',
                    color: '#fff'
                  }}>
                    {analyzedMedia.metadata.extractor.toUpperCase()}
                  </div>
                </div>

                {/* Right Side: Options & Preset Picker */}
                <div style={{ flex: 1, display: 'flex', flexDirection: 'column', justifyContent: 'space-between' }}>
                  <div>
                    <h3 style={{ fontSize: '1.2rem', fontWeight: 700, marginBottom: '0.5rem', lineHeight: '1.3', color: '#fff' }}>
                      {analyzedMedia.metadata.title}
                    </h3>
                    <p style={{ color: 'var(--text-sub)', fontSize: '0.85rem', marginBottom: '1.25rem' }}>
                      By {analyzedMedia.metadata.uploader || 'Unknown Creator'}
                    </p>

                    {/* Format & Quality Selector */}
                    <div style={{ display: 'flex', flexDirection: 'column', gap: '0.85rem', marginBottom: '1.25rem' }}>
                      {/* 1. Download Type Selector */}
                      <div style={{ display: 'flex', flexDirection: 'column', gap: '0.35rem' }}>
                        <label style={{ fontSize: '0.75rem', color: 'var(--text-sub)', fontWeight: 600 }}>DOWNLOAD TYPE</label>
                        <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '0.5rem' }}>
                          <button 
                            type="button" 
                            className={downloadType === 'video' ? 'btn-primary' : 'btn-secondary'} 
                            style={{ padding: '0.45rem', fontSize: '0.75rem', border: downloadType === 'video' ? '1px solid var(--accent-indigo)' : '1px solid var(--border-slate)' }}
                            onClick={() => setDownloadType('video')}
                          >
                            Video File
                          </button>
                          <button 
                            type="button" 
                            className={downloadType === 'audio' ? 'btn-primary' : 'btn-secondary'} 
                            style={{ padding: '0.45rem', fontSize: '0.75rem', border: downloadType === 'audio' ? '1px solid var(--accent-indigo)' : '1px solid var(--border-slate)' }}
                            onClick={() => setDownloadType('audio')}
                          >
                            Audio Extraction
                          </button>
                        </div>
                      </div>

                      {downloadType === 'video' ? (
                        <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '1rem' }}>
                          {/* Video Format Selection */}
                          <div style={{ display: 'flex', flexDirection: 'column', gap: '0.35rem' }}>
                            <label style={{ fontSize: '0.75rem', color: 'var(--text-sub)', fontWeight: 600 }}>FORMAT</label>
                            <select 
                              className="cyber-input"
                              value={videoFormat}
                              onChange={(e: any) => setVideoFormat(e.target.value)}
                            >
                              <option value="mp4">.mp4 (Universal)</option>
                              <option value="mkv">.mkv (Raw Merged)</option>
                            </select>
                          </div>

                          {/* Video Quality Selection */}
                          <div style={{ display: 'flex', flexDirection: 'column', gap: '0.35rem' }}>
                            <label style={{ fontSize: '0.75rem', color: 'var(--text-sub)', fontWeight: 600 }}>QUALITY OPTIONS</label>
                            <select 
                              className="cyber-input"
                              value={videoQuality}
                              onChange={(e: any) => setVideoQuality(e.target.value)}
                            >
                              {getDetectedQualities().map((q) => (
                                <option key={q.value} value={q.value}>
                                  {q.label}
                                </option>
                              ))}
                            </select>
                          </div>
                        </div>
                      ) : (
                        /* Audio Format Selection */
                        <div style={{ display: 'flex', flexDirection: 'column', gap: '0.35rem' }}>
                          <label style={{ fontSize: '0.75rem', color: 'var(--text-sub)', fontWeight: 600 }}>FORMAT</label>
                          <select 
                            className="cyber-input"
                            value={audioFormat}
                            onChange={(e: any) => setAudioFormat(e.target.value)}
                          >
                            <option value="mp3">.mp3 (Universal 320kbps)</option>
                            <option value="wav">.wav (Lossless Studio Waveform)</option>
                          </select>
                        </div>
                      )}
                    </div>

                    {/* Advanced Cookie/Browser Selection (Available in Advanced Mode) */}
                    {advancedMode && analyzedMedia.capabilities.supports_login && (
                      <div style={{ display: 'flex', flexDirection: 'column', gap: '0.5rem', marginTop: '0.75rem' }}>
                        <label style={{ fontSize: '0.8rem', color: 'var(--text-sub)', fontWeight: 600 }}>BROWSER TO LOAD LOGIN COOKIES FROM</label>
                        <select 
                          className="cyber-input"
                          value={settings.cookies_browser}
                          onChange={(e) => handleSaveSettings({ ...settings, cookies_browser: e.target.value })}
                        >
                          <option value="">None (Public Content Only)</option>
                          <option value="chrome">Google Chrome</option>
                          <option value="firefox">Mozilla Firefox</option>
                          <option value="safari">Apple Safari</option>
                          <option value="edge">Microsoft Edge</option>
                        </select>
                      </div>
                    )}
                  </div>

                  {/* Action buttons */}
                  <div style={{ display: 'flex', gap: '1rem' }}>
                    <button className="btn-secondary" style={{ flex: 1 }} onClick={() => setAnalyzedMedia(null)}>
                      Cancel
                    </button>
                    <button className="btn-primary" style={{ flex: 2 }} onClick={handleStartDownload}>
                      <Download size={18} />
                      Download Now
                    </button>
                  </div>
                </div>
              </div>
            )}
            
            {/* Guide Card (if empty) */}
            {!analyzedMedia && (
              <div className="cyber-card" style={{ display: 'flex', flexDirection: 'column', gap: '1rem', alignItems: 'center', justifyContent: 'center', flex: 1, borderStyle: 'dashed', background: 'transparent' }}>
                <div style={{ width: '56px', height: '56px', borderRadius: '50%', background: 'rgba(255, 255, 255, 0.02)', display: 'flex', alignItems: 'center', justifyContent: 'center', border: '1px solid var(--border-slate)' }}>
                  <Download size={24} color="var(--text-sub)" />
                </div>
                <div style={{ textAlign: 'center' }}>
                  <h3 style={{ fontSize: '0.95rem', color: 'var(--text-main)', marginBottom: '0.25rem' }}>Ready to Download</h3>
                  <p style={{ fontSize: '0.8rem', color: 'var(--text-muted)' }}>Paste any link in the bar above and click "Scan Link" to pick resolutions.</p>
                </div>
              </div>
            )}
          </div>
        )}

        {/* 2. Active Queue Tab */}
        {activeTab === 'queue' && (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '1.5rem', flex: 1 }}>
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
              <h2 style={{ fontSize: '1.25rem', fontWeight: 700 }}>ACTIVE DOWNLOADS</h2>
              <button className="btn-danger" style={{ padding: '0.5rem 1rem', fontSize: '0.75rem' }} onClick={handleClearQueue} disabled={queue.length === 0}>
                <Trash2 size={14} />
                Clear Completed
              </button>
            </div>

            {/* List */}
            <div style={{ display: 'flex', flexDirection: 'column', gap: '1rem', overflowY: 'auto', flex: 1 }}>
              {queue.filter((t) => t.status !== 'Completed').length === 0 ? (
                <div className="cyber-card" style={{ display: 'flex', flexDirection: 'column', gap: '1rem', alignItems: 'center', justifyContent: 'center', height: '240px', borderStyle: 'dashed', background: 'transparent' }}>
                  <Activity size={28} color="var(--text-muted)" />
                  <p style={{ fontSize: '0.85rem', color: 'var(--text-muted)' }}>There are no active downloads at the moment.</p>
                </div>
              ) : (
                queue.filter((t) => t.status !== 'Completed').map((task) => (
                  <div key={task.id} className="cyber-card" style={{ padding: '1.25rem' }}>
                    <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start', marginBottom: '0.75rem' }}>
                      <div style={{ flex: 1, minWidth: 0, paddingRight: '1rem' }}>
                        <h4 style={{ fontSize: '0.95rem', fontWeight: 600, marginBottom: '0.25rem', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                          {task.file_path ? task.file_path.split('/').pop() : task.url}
                        </h4>
                        <div style={{ display: 'flex', gap: '0.75rem', fontSize: '0.75rem', color: 'var(--text-sub)', alignItems: 'center' }}>
                          <span style={{ background: 'rgba(255, 255, 255, 0.05)', padding: '1px 6px', borderRadius: '4px', fontSize: '0.7rem' }}>
                            {task.task_type.type === 'DownloadVideo' ? 'VIDEO' : 'AUDIO-ONLY'}
                          </span>
                          <span style={{ 
                            color: task.status === 'Downloading' ? 'var(--accent-green)' : 
                                   task.status === 'Pending' ? 'var(--accent-indigo)' :
                                   task.status === 'Completed' ? 'var(--accent-green)' :
                                   task.status === 'Failed' ? 'var(--accent-rose)' : 'var(--accent-amber)',
                            fontWeight: 700 
                          }}>
                            {task.status}
                          </span>
                        </div>
                      </div>

                      {/* Controls */}
                      <div style={{ display: 'flex', gap: '0.5rem', flexShrink: 0 }}>
                        {task.status === 'Downloading' && (
                          <button className="btn-secondary" style={{ padding: '0.4rem 0.85rem', fontSize: '0.7rem' }} onClick={() => handleCancelDownload(task.id)}>
                            Pause
                          </button>
                        )}
                        {(task.status === 'Paused' || task.status === 'Interrupted' || task.status === 'Failed') && (
                          <button className="btn-primary" style={{ padding: '0.4rem 0.85rem', fontSize: '0.7rem' }} onClick={() => handleResumeDownload(task.id)}>
                            Resume
                          </button>
                        )}
                        <button className="btn-danger" style={{ padding: '0.45rem', borderRadius: '8px' }} onClick={() => handleDeleteTask(task.id)}>
                          <Trash2 size={14} />
                        </button>
                      </div>
                    </div>

                    {/* Progress Fill */}
                    {task.status === 'Downloading' && (
                      <div style={{ display: 'flex', flexDirection: 'column', gap: '0.4rem' }}>
                        <div className="progress-bar-container">
                          <div className="progress-bar-fill" style={{ width: `${task.progress}%` }}></div>
                        </div>
                        <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: '0.75rem', color: 'var(--text-sub)' }} className="mono-metric">
                          <span>{task.progress.toFixed(1)}%</span>
                          <div style={{ display: 'flex', gap: '1rem' }}>
                            {task.speed && <span>{task.speed}</span>}
                            {task.eta && <span>{task.eta} left</span>}
                          </div>
                        </div>
                      </div>
                    )}

                    {/* Error display */}
                    {task.status === 'Failed' && task.error_msg && (
                      <div style={{ background: 'rgba(244, 63, 94, 0.03)', border: '1px solid rgba(244, 63, 94, 0.15)', padding: '0.6rem 0.85rem', borderRadius: '8px', fontSize: '0.75rem', color: 'var(--accent-rose)', lineHeight: '1.4' }}>
                        Issue: {task.error_msg}
                      </div>
                    )}
                  </div>
                ))
              )}
            </div>
          </div>
        )}

        {/* 3. History Logs Tab */}
        {activeTab === 'history' && (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '1.5rem', flex: 1 }}>
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
              <h2 style={{ fontSize: '1.25rem', fontWeight: 700 }}>SAVED LIBRARY</h2>
              <button className="btn-danger" style={{ padding: '0.5rem 1rem', fontSize: '0.75rem' }} onClick={handleClearHistory} disabled={history.length === 0}>
                <Trash2 size={14} />
                Clear History
              </button>
            </div>

            {/* Search Input */}
            <div style={{ display: 'flex', gap: '0.75rem' }}>
              <div style={{ flex: 1, position: 'relative' }}>
                <input 
                  type="text" 
                  className="cyber-input" 
                  style={{ width: '100%', paddingLeft: '2.75rem' }} 
                  placeholder="Search previously downloaded files..." 
                  value={searchQuery}
                  onChange={(e) => setSearchQuery(e.target.value)}
                  onKeyDown={(e) => e.key === 'Enter' && fetchHistory()}
                />
                <Search size={16} color="var(--text-muted)" style={{ position: 'absolute', left: '1.15rem', top: '50%', transform: 'translateY(-50%)' }} />
              </div>
              <button className="btn-secondary" style={{ padding: '0.75rem 1.5rem' }} onClick={fetchHistory}>
                Search
              </button>
            </div>

            {/* List */}
            <div style={{ display: 'flex', flexDirection: 'column', gap: '1rem', overflowY: 'auto', flex: 1 }}>
              {history.length === 0 ? (
                <div className="cyber-card" style={{ display: 'flex', flexDirection: 'column', gap: '1rem', alignItems: 'center', justifyContent: 'center', height: '240px', borderStyle: 'dashed', background: 'transparent' }}>
                  <Layers size={28} color="var(--text-muted)" />
                  <p style={{ fontSize: '0.85rem', color: 'var(--text-muted)' }}>You haven't downloaded any files yet.</p>
                </div>
              ) : (
                history.map((item) => (
                  <div key={item.id} className="cyber-card" style={{ padding: '1rem 1.25rem', display: 'flex', gap: '1.25rem', alignItems: 'center' }}>
                    
                    {/* Tiny Thumbnail */}
                    <div style={{ width: '72px', height: '48px', background: '#090b11', borderRadius: '6px', overflow: 'hidden', flexShrink: 0, border: '1px solid var(--border-slate)' }}>
                      {item.thumbnail_path ? (
                        <img src={item.thumbnail_path} alt="Thumb" style={{ width: '100%', height: '100%', objectFit: 'cover' }} />
                      ) : (
                        <div style={{ display: 'flex', width: '100%', height: '100%', alignItems: 'center', justifyContent: 'center' }}>
                          <Play size={16} color="rgba(255,255,255,0.1)" />
                        </div>
                      )}
                    </div>

                    {/* Middle Metadata block */}
                    <div style={{ flex: 1, minWidth: 0 }}>
                      <h4 style={{ fontSize: '0.9rem', fontWeight: 600, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', marginBottom: '0.25rem' }}>
                        {item.title}
                      </h4>
                      <div style={{ display: 'flex', gap: '0.8rem', fontSize: '0.75rem', color: 'var(--text-sub)', alignItems: 'center' }} className="mono-metric">
                        <span style={{ color: 'var(--accent-indigo)', fontWeight: 700 }}>{item.source_site?.toUpperCase()}</span>
                        <span>{formatBytes(item.file_size)}</span>
                        <span>{formatDuration(item.duration)}</span>
                        <span>{item.completed_at ? new Date(item.completed_at).toLocaleDateString() : ''}</span>
                      </div>
                    </div>

                    {/* Action buttons */}
                    <div style={{ display: 'flex', gap: '0.5rem', flexShrink: 0 }}>
                      <button className="btn-secondary" style={{ padding: '0.45rem 1rem', fontSize: '0.75rem', display: 'flex', gap: '6px' }} onClick={() => handleOpenFolder(item.file_path)}>
                        <FolderOpen size={14} />
                        Locate File
                      </button>
                      <button className="btn-danger" style={{ padding: '0.45rem', borderRadius: '8px' }} onClick={() => handleDeleteHistoryItem(item.id)}>
                        <Trash2 size={14} />
                      </button>
                    </div>

                  </div>
                ))
              )}
            </div>
          </div>
        )}

        {/* 4. Settings & Diagnostics Tab */}
        {activeTab === 'settings' && (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '1.75rem', flex: 1, overflowY: 'auto' }}>
            <div>
              <h2 style={{ fontSize: '1.25rem', fontWeight: 700, marginBottom: '0.25rem' }}>APPLICATION SETTINGS</h2>
              <p style={{ fontSize: '0.8rem', color: 'var(--text-sub)' }}>Configure folder locations, maximum downloads, and check system health.</p>
            </div>

            {/* Advanced Toggle */}
            <div className="cyber-card" style={{ padding: '1.15rem 1.5rem', display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
              <div style={{ display: 'flex', gap: '0.85rem', alignItems: 'center' }}>
                <Sliders size={20} color="var(--accent-indigo)" />
                <div>
                  <h4 style={{ fontSize: '0.9rem', fontWeight: 700 }}>Show Advanced Settings</h4>
                  <p style={{ fontSize: '0.75rem', color: 'var(--text-muted)' }}>Exposes cookie extractions, diagnostic reports, and custom engine settings.</p>
                </div>
              </div>
              <div style={{ cursor: 'pointer' }} onClick={() => handleSaveSettings({ ...settings, advanced_mode: !advancedMode })}>
                {advancedMode ? (
                  <ToggleRight size={38} color="var(--accent-indigo)" />
                ) : (
                  <ToggleLeft size={38} color="var(--text-muted)" />
                )}
              </div>
            </div>

            {/* Form Fields */}
            <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '1.5rem' }}>
              
              {/* Output Directory */}
              <div style={{ display: 'flex', flexDirection: 'column', gap: '0.4rem' }}>
                <label style={{ fontSize: '0.8rem', color: 'var(--text-sub)', fontWeight: 600 }}>DOWNLOAD FOLDER DESTINATION</label>
                <input 
                  type="text" 
                  className="cyber-input" 
                  value={settings.download_directory}
                  onChange={(e) => handleSaveSettings({ ...settings, download_directory: e.target.value })}
                />
              </div>

              {/* Concurrency Limit */}
              <div style={{ display: 'flex', flexDirection: 'column', gap: '0.4rem' }}>
                <label style={{ fontSize: '0.8rem', color: 'var(--text-sub)', fontWeight: 600 }}>MAX SIMULTANEOUS DOWNLOADS</label>
                <select 
                  className="cyber-input"
                  value={settings.max_concurrent_tasks}
                  onChange={(e) => handleSaveSettings({ ...settings, max_concurrent_tasks: parseInt(e.target.value) })}
                >
                  <option value="1">1 Download (Optimal for most connections)</option>
                  <option value="2">2 Downloads (Recommended)</option>
                  <option value="3">3 Downloads</option>
                  <option value="4">4 Downloads</option>
                </select>
              </div>

              {/* Duplicate checking */}
              <div style={{ display: 'flex', flexDirection: 'column', gap: '0.4rem' }}>
                <label style={{ fontSize: '0.8rem', color: 'var(--text-sub)', fontWeight: 600 }}>DUPLICATE DETECT MODE</label>
                <select 
                  className="cyber-input"
                  value={settings.skip_previously_downloaded.toString()}
                  onChange={(e) => handleSaveSettings({ ...settings, skip_previously_downloaded: e.target.value === 'true' })}
                >
                  <option value="false">Append numbers to duplicates (e.g. video (1))</option>
                  <option value="true">Skip previously downloaded (archive database)</option>
                </select>
              </div>

              {/* Scraper updates channel */}
              <div style={{ display: 'flex', flexDirection: 'column', gap: '0.4rem' }}>
                <label style={{ fontSize: '0.8rem', color: 'var(--text-sub)', fontWeight: 600 }}>ENGINE STABILITY CHANNEL</label>
                <select 
                  className="cyber-input"
                  value={settings.yt_dlp_channel}
                  onChange={(e) => handleSaveSettings({ ...settings, yt_dlp_channel: e.target.value })}
                >
                  <option value="Stable">Stable releases (Highly recommended)</option>
                  <option value="Beta">Beta builds</option>
                  <option value="Nightly">Nightly builds (Fast extractor fixes)</option>
                </select>
              </div>

            </div>

            {/* Advanced Section */}
            {advancedMode && (
              <div style={{ display: 'flex', flexDirection: 'column', gap: '1.25rem', borderTop: '1px solid var(--border-slate)', paddingTop: '1.5rem' }}>
                <h3 style={{ fontSize: '0.95rem', color: 'var(--accent-indigo)', fontWeight: 700 }}>ADVANCED PARAMETERS</h3>
                
                <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '1.5rem' }}>
                  {/* Custom yt-dlp path */}
                  <div style={{ display: 'flex', flexDirection: 'column', gap: '0.4rem' }}>
                    <label style={{ fontSize: '0.8rem', color: 'var(--text-sub)', fontWeight: 600 }}>CUSTOM YT-DLP EXECUTABLE PATH</label>
                    <input 
                      type="text" 
                      className="cyber-input" 
                      placeholder="Leave blank to use built-in bundler..."
                      value={settings.custom_yt_dlp_path}
                      onChange={(e) => handleSaveSettings({ ...settings, custom_yt_dlp_path: e.target.value })}
                    />
                  </div>

                  {/* Custom flags */}
                  <div style={{ display: 'flex', flexDirection: 'column', gap: '0.4rem' }}>
                    <label style={{ fontSize: '0.8rem', color: 'var(--text-sub)', fontWeight: 600 }}>APPEND CUSTOM YT-DLP CLI FLAGS</label>
                    <input 
                      type="text" 
                      className="cyber-input" 
                      placeholder="e.g. --embed-subs --restrict-filenames"
                      value={settings.custom_yt_dlp_flags}
                      onChange={(e) => handleSaveSettings({ ...settings, custom_yt_dlp_flags: e.target.value })}
                    />
                  </div>
                </div>

                {/* Diagnostics block */}
                {selfCheck && (
                  <div className="cyber-card" style={{ display: 'flex', flexDirection: 'column', gap: '0.75rem', marginTop: '0.5rem' }}>
                    <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', borderBottom: '1px solid rgba(255,255,255,0.05)', paddingBottom: '0.5rem', marginBottom: '0.25rem' }}>
                      <h4 style={{ fontSize: '0.85rem', fontWeight: 700 }}>SYSTEM HEALTH & DIAGNOSTICS</h4>
                      <button className="btn-secondary" style={{ padding: '0.35rem 0.85rem', fontSize: '0.7rem' }} onClick={triggerSelfCheck}>
                        Refresh Checks
                      </button>
                    </div>
                    <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '1rem', fontSize: '0.75rem' }} className="mono-metric">
                      <div>
                        <p><strong style={{ color: 'var(--text-sub)' }}>Scraper Status:</strong> {selfCheck.yt_dlp.status}</p>
                        <p><strong style={{ color: 'var(--text-sub)' }}>Scraper Path:</strong> {selfCheck.yt_dlp.path || 'None'}</p>
                        <p><strong style={{ color: 'var(--text-sub)' }}>Scraper Version:</strong> {selfCheck.yt_dlp.version || 'Unknown'}</p>
                      </div>
                      <div>
                        <p><strong style={{ color: 'var(--text-sub)' }}>ffmpeg Status:</strong> {selfCheck.ffmpeg.status}</p>
                        <p><strong style={{ color: 'var(--text-sub)' }}>ffmpeg Path:</strong> {selfCheck.ffmpeg.path || 'None'}</p>
                        <p><strong style={{ color: 'var(--text-sub)' }}>SQLite DB Integrity:</strong> {selfCheck.database}</p>
                      </div>
                    </div>
                    <div style={{ display: 'flex', justifyContent: 'flex-end', gap: '0.75rem', marginTop: '0.5rem' }}>
                      <button className="btn-primary" style={{ fontSize: '0.7rem', padding: '0.45rem 1rem' }} onClick={handleRepairBinaries} disabled={isRepairing}>
                        <RefreshCw size={12} className={isRepairing ? 'spin-anim' : ''} />
                        {isRepairing ? 'Updating Engine...' : 'Check For Scraper Updates'}
                      </button>
                    </div>
                  </div>
                )}
              </div>
            )}
          </div>
        )}

      </main>

      {/* Toast Notification Popups */}
      {toast && (
        <div className="cyber-toast" style={{ borderColor: toast.type === 'success' ? 'var(--accent-green)' : toast.type === 'error' ? 'var(--accent-rose)' : 'var(--accent-indigo)' }}>
          {toast.type === 'success' && <CheckCircle size={18} color="var(--accent-green)" />}
          {toast.type === 'error' && <AlertTriangle size={18} color="var(--accent-rose)" />}
          {toast.type === 'info' && <Cpu size={18} color="var(--accent-indigo)" />}
          <span style={{ fontSize: '0.8rem', fontWeight: 600 }}>{toast.message}</span>
        </div>
      )}

    </div>
  );
}
