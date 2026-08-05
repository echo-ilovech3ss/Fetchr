import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { describe, it, expect } from 'vitest';
import App from '../App';

describe('App Main Component', () => {
  it('renders application sidebar header and navigation tabs', async () => {
    render(<App />);

    await waitFor(() => {
      expect(screen.getByText('Video Saver')).toBeInTheDocument();
    });

    expect(screen.getByText('Download Media')).toBeInTheDocument();
    expect(screen.getByText('Active Downloads')).toBeInTheDocument();
    expect(screen.getByText('Saved Library')).toBeInTheDocument();
    expect(screen.getByText('App Settings')).toBeInTheDocument();
  });

  it('allows user to type video links into downloader bar', async () => {
    render(<App />);

    const input = screen.getByPlaceholderText(/Paste any YouTube video link/i);
    fireEvent.change(input, { target: { value: 'https://www.instagram.com/reel/C123456789/' } });

    expect(input).toHaveValue('https://www.instagram.com/reel/C123456789/');
    expect(screen.getByText('Instagram Reel / Post')).toBeInTheDocument();
  });

  it('switches navigation tabs when clicked', async () => {
    render(<App />);

    // Click Active Downloads tab
    const queueTab = screen.getByText('Active Downloads');
    fireEvent.click(queueTab);

    expect(screen.getByText('Clear Completed')).toBeInTheDocument();

    // Click Saved Library tab
    const historyTab = screen.getByText('Saved Library');
    fireEvent.click(historyTab);

    expect(screen.getByText('Clear History')).toBeInTheDocument();

    // Click Settings tab
    const settingsTab = screen.getByText('App Settings');
    fireEvent.click(settingsTab);

    expect(screen.getByText('Application Settings')).toBeInTheDocument();
  });

  it('renders 3D canvas pipeline on empty downloader screen', async () => {
    render(<App />);
    expect(screen.getByText(/Engine Core • 3D Interactive Pipeline/i)).toBeInTheDocument();
    expect(screen.getByTestId('three-canvas-container')).toBeInTheDocument();
  });
});
