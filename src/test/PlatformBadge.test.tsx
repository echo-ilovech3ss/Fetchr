import { render, screen } from '@testing-library/react';
import { describe, it, expect } from 'vitest';
import PlatformBadge, { detectPlatformFromUrl } from '../components/PlatformBadge';

describe('detectPlatformFromUrl', () => {
  it('detects YouTube links correctly', () => {
    expect(detectPlatformFromUrl('https://www.youtube.com/watch?v=dQw4w9WgXcQ')).toBe('youtube');
    expect(detectPlatformFromUrl('https://youtu.be/dQw4w9WgXcQ')).toBe('youtube');
  });

  it('detects Instagram links correctly', () => {
    expect(detectPlatformFromUrl('https://www.instagram.com/reel/C12345/')).toBe('instagram');
    expect(detectPlatformFromUrl('https://www.instagram.com/p/C12345/')).toBe('instagram');
    expect(detectPlatformFromUrl('https://instagr.am/p/C12345/')).toBe('instagram');
  });

  it('detects Facebook links correctly', () => {
    expect(detectPlatformFromUrl('https://www.facebook.com/reel/987654/')).toBe('facebook');
    expect(detectPlatformFromUrl('https://www.facebook.com/watch/?v=987654')).toBe('facebook');
    expect(detectPlatformFromUrl('https://fb.watch/xyz123/')).toBe('facebook');
  });

  it('detects TikTok links correctly', () => {
    expect(detectPlatformFromUrl('https://www.tiktok.com/@user/video/123456')).toBe('tiktok');
  });

  it('detects Vimeo links correctly', () => {
    expect(detectPlatformFromUrl('https://vimeo.com/12345678')).toBe('vimeo');
  });

  it('defaults to generic for unknown links', () => {
    expect(detectPlatformFromUrl('https://mywebsite.com/video.mp4')).toBe('generic');
  });
});

describe('PlatformBadge Component', () => {
  it('renders nothing when url is empty', () => {
    const { container } = render(<PlatformBadge url="" />);
    expect(container.firstChild).toBeNull();
  });

  it('renders Instagram badge with bot bypass tip', () => {
    render(<PlatformBadge url="https://www.instagram.com/reel/C12345/" />);
    expect(screen.getByText('Instagram Reel / Post')).toBeInTheDocument();
    expect(screen.getByText(/Bot-bypass enabled/i)).toBeInTheDocument();
  });

  it('renders Facebook badge with normalization tip', () => {
    render(<PlatformBadge url="https://www.facebook.com/reel/987654/" />);
    expect(screen.getByText('Facebook Video / Reel')).toBeInTheDocument();
    expect(screen.getByText(/Facebook link normalized/i)).toBeInTheDocument();
  });

  it('renders YouTube badge', () => {
    render(<PlatformBadge url="https://www.youtube.com/watch?v=dQw4w9WgXcQ" />);
    expect(screen.getByText('YouTube')).toBeInTheDocument();
  });

  it('renders TikTok badge', () => {
    render(<PlatformBadge url="https://www.tiktok.com/@user/video/123" />);
    expect(screen.getByText('TikTok Video')).toBeInTheDocument();
  });

  it('renders Vimeo badge', () => {
    render(<PlatformBadge url="https://vimeo.com/12345" />);
    expect(screen.getByText('Vimeo')).toBeInTheDocument();
  });
});
