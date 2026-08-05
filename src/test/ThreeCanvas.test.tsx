import { render, screen } from '@testing-library/react';
import { describe, it, expect } from 'vitest';
import ThreeCanvas from '../components/ThreeCanvas';

describe('ThreeCanvas Component', () => {
  it('renders ThreeCanvas container in DOM', () => {
    render(<ThreeCanvas mode="ambient" height="200px" />);
    const container = screen.getByTestId('three-canvas-container');
    expect(container).toBeInTheDocument();
    expect(container).toHaveStyle({ height: '200px' });
  });

  it('renders correctly with different modes', () => {
    const { rerender } = render(<ThreeCanvas mode="scanning" />);
    expect(screen.getByTestId('three-canvas-container')).toBeInTheDocument();

    rerender(<ThreeCanvas mode="downloading" />);
    expect(screen.getByTestId('three-canvas-container')).toBeInTheDocument();
  });
});
