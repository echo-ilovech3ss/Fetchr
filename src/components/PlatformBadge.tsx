import { Video, Film, Link as LinkIcon, ShieldAlert } from 'lucide-react';

interface PlatformBadgeProps {
  url: string;
}

export type PlatformType = 'youtube' | 'instagram' | 'facebook' | 'tiktok' | 'vimeo' | 'generic';

export function detectPlatformFromUrl(url: string): PlatformType {
  const lower = url.toLowerCase();
  if (lower.includes('youtube.com') || lower.includes('youtu.be')) return 'youtube';
  if (lower.includes('instagram.com') || lower.includes('instagr.am')) return 'instagram';
  if (lower.includes('facebook.com') || lower.includes('fb.watch') || lower.includes('fb.com')) return 'facebook';
  if (lower.includes('tiktok.com')) return 'tiktok';
  if (lower.includes('vimeo.com')) return 'vimeo';
  return 'generic';
}

function YoutubeIcon({ size = 14, color = 'currentColor' }: { size?: number; color?: string }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke={color} strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M22.54 6.42a2.78 2.78 0 0 0-1.94-2C18.88 4 12 4 12 4s-6.88 0-8.6.46a2.78 2.78 0 0 0-1.94 2A29 29 0 0 0 1 11.75a29 29 0 0 0 .46 5.33A2.78 2.78 0 0 0 3.4 19c1.72.46 8.6.46 8.6.46s6.88 0 8.6-.46a2.78 2.78 0 0 0 1.94-2 29 29 0 0 0 .46-5.25 29 29 0 0 0-.46-5.33z"></path>
      <polygon points="9.75 15.02 15.5 11.75 9.75 8.48 9.75 15.02"></polygon>
    </svg>
  );
}

function InstagramIcon({ size = 14, color = 'currentColor' }: { size?: number; color?: string }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke={color} strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <rect x="2" y="2" width="20" height="20" rx="5" ry="5"></rect>
      <path d="M16 11.37A4 4 0 1 1 12.63 8 4 4 0 0 1 16 11.37z"></path>
      <line x1="17.5" y1="6.5" x2="17.51" y2="6.5"></line>
    </svg>
  );
}

function FacebookIcon({ size = 14, color = 'currentColor' }: { size?: number; color?: string }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke={color} strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M18 2h-3a5 5 0 0 0-5 5v3H7v4h3v8h4v-8h3l1-4h-4V7a1 1 0 0 1 1-1h3z"></path>
    </svg>
  );
}

export default function PlatformBadge({ url }: PlatformBadgeProps) {
  if (!url || !url.trim()) return null;

  const platform = detectPlatformFromUrl(url);

  const configs: Record<PlatformType, { name: string; bg: string; border: string; color: string; icon: any; tip?: string }> = {
    youtube: {
      name: 'YouTube',
      bg: 'rgba(255, 0, 0, 0.12)',
      border: 'rgba(255, 0, 0, 0.3)',
      color: '#ff4d4d',
      icon: YoutubeIcon,
    },
    instagram: {
      name: 'Instagram Reel / Post',
      bg: 'rgba(225, 48, 108, 0.12)',
      border: 'rgba(225, 48, 108, 0.35)',
      color: '#e1306c',
      icon: InstagramIcon,
      tip: 'Bot-bypass enabled. If downloading a private Reel, enable browser cookies in Settings.',
    },
    facebook: {
      name: 'Facebook Video / Reel',
      bg: 'rgba(24, 119, 242, 0.12)',
      border: 'rgba(24, 119, 242, 0.35)',
      color: '#1877f2',
      icon: FacebookIcon,
      tip: 'Facebook link normalized. Standard & HD streams will be automatically extracted.',
    },
    tiktok: {
      name: 'TikTok Video',
      bg: 'rgba(0, 242, 234, 0.12)',
      border: 'rgba(0, 242, 234, 0.3)',
      color: '#00f2ea',
      icon: Video,
    },
    vimeo: {
      name: 'Vimeo',
      bg: 'rgba(26, 183, 234, 0.12)',
      border: 'rgba(26, 183, 234, 0.3)',
      color: '#1ab7ea',
      icon: Film,
    },
    generic: {
      name: 'Direct Media Link',
      bg: 'rgba(235, 220, 210, 0.08)',
      border: 'rgba(235, 220, 210, 0.2)',
      color: '#ebdcd2',
      icon: LinkIcon,
    },
  };

  const config = configs[platform];
  const IconComponent = config.icon;

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '0.4rem', marginTop: '0.4rem' }}>
      <div
        data-testid="platform-badge"
        style={{
          display: 'inline-flex',
          alignItems: 'center',
          gap: '0.4rem',
          padding: '4px 10px',
          borderRadius: '4px',
          background: config.bg,
          border: `1px solid ${config.border}`,
          color: config.color,
          fontSize: '0.75rem',
          fontWeight: 700,
          width: 'fit-content',
        }}
      >
        <IconComponent size={14} color={config.color} />
        <span>{config.name}</span>
      </div>

      {config.tip && (
        <div
          style={{
            display: 'flex',
            alignItems: 'center',
            gap: '0.4rem',
            fontSize: '0.72rem',
            color: 'var(--text-sub)',
            background: 'rgba(255,255,255,0.02)',
            padding: '4px 8px',
            borderRadius: '4px',
            border: '1px solid rgba(255,255,255,0.05)',
          }}
        >
          <ShieldAlert size={12} color="var(--accent-yellow)" />
          <span>{config.tip}</span>
        </div>
      )}
    </div>
  );
}
