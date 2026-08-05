import { useEffect, useRef } from 'react';
import * as THREE from 'three';

interface ThreeCanvasProps {
  mode?: 'ambient' | 'scanning' | 'downloading';
  height?: string | number;
  interactive?: boolean;
}

export default function ThreeCanvas({
  mode = 'ambient',
  height = '100%',
  interactive = true,
}: ThreeCanvasProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const modeRef = useRef(mode);
  modeRef.current = mode;

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    // Check WebGL support in environment
    const testCanvas = document.createElement('canvas');
    const gl = testCanvas.getContext('webgl') || testCanvas.getContext('experimental-webgl');
    if (!gl || typeof (gl as any).texImage3D !== 'function') {
      return;
    }

    const width = container.clientWidth || 300;
    const currentHeight = container.clientHeight || 300;

    // 1. Scene Setup
    const scene = new THREE.Scene();

    // 2. Camera Setup
    const camera = new THREE.PerspectiveCamera(60, width / currentHeight, 0.1, 1000);
    camera.position.z = 5;

    // 3. Renderer Setup
    const renderer = new THREE.WebGLRenderer({
      alpha: true,
      antialias: true,
      powerPreference: 'high-performance',
    });
    renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
    renderer.setSize(width, currentHeight);
    container.appendChild(renderer.domElement);

    // 4. Create 3D Objects
    // A. Central Geometric Icosahedron
    const icoGeometry = new THREE.IcosahedronGeometry(1.4, 1);
    const icoMaterial = new THREE.MeshBasicMaterial({
      color: 0xe05c3b,
      wireframe: true,
      transparent: true,
      opacity: 0.45,
    });
    const icoMesh = new THREE.Mesh(icoGeometry, icoMaterial);
    scene.add(icoMesh);

    // B. Inner Core Sphere
    const coreGeometry = new THREE.SphereGeometry(0.8, 16, 16);
    const coreMaterial = new THREE.MeshBasicMaterial({
      color: 0xebdcd2,
      wireframe: true,
      transparent: true,
      opacity: 0.25,
    });
    const coreMesh = new THREE.Mesh(coreGeometry, coreMaterial);
    scene.add(coreMesh);

    // C. Particle Swarm Field
    const particleCount = 200;
    const particlePositions = new Float32Array(particleCount * 3);
    const particleScales = new Float32Array(particleCount);

    for (let i = 0; i < particleCount; i++) {
      const radius = 2.5 + Math.random() * 2.5;
      const theta = Math.random() * Math.PI * 2;
      const phi = Math.acos(Math.random() * 2 - 1);

      particlePositions[i * 3] = radius * Math.sin(phi) * Math.cos(theta);
      particlePositions[i * 3 + 1] = radius * Math.sin(phi) * Math.sin(theta);
      particlePositions[i * 3 + 2] = radius * Math.cos(phi);

      particleScales[i] = Math.random();
    }

    const particleGeometry = new THREE.BufferGeometry();
    particleGeometry.setAttribute('position', new THREE.BufferAttribute(particlePositions, 3));

    const particleMaterial = new THREE.PointsMaterial({
      color: 0xe05c3b,
      size: 0.05,
      transparent: true,
      opacity: 0.7,
      blending: THREE.AdditiveBlending,
    });

    const particles = new THREE.Points(particleGeometry, particleMaterial);
    scene.add(particles);

    // 5. Mouse Parallax Interactions
    let mouseX = 0;
    let mouseY = 0;
    let targetX = 0;
    let targetY = 0;

    const handleMouseMove = (e: MouseEvent) => {
      if (!interactive) return;
      const rect = container.getBoundingClientRect();
      mouseX = ((e.clientX - rect.left) / rect.width) * 2 - 1;
      mouseY = -(((e.clientY - rect.top) / rect.height) * 2 - 1);
    };

    if (interactive) {
      window.addEventListener('mousemove', handleMouseMove);
    }

    // 6. Animation Loop
    let animationFrameId: number;
    let clock = new THREE.Clock();

    const animate = () => {
      animationFrameId = requestAnimationFrame(animate);
      const elapsedTime = clock.getElapsedTime();

      targetX += (mouseX - targetX) * 0.05;
      targetY += (mouseY - targetY) * 0.05;

      const currentMode = modeRef.current;
      const speedMultiplier = currentMode === 'scanning' ? 2.8 : currentMode === 'downloading' ? 1.8 : 1.0;

      icoMesh.rotation.x = elapsedTime * 0.25 * speedMultiplier + targetY * 0.5;
      icoMesh.rotation.y = elapsedTime * 0.35 * speedMultiplier + targetX * 0.5;

      coreMesh.rotation.x = -elapsedTime * 0.4 * speedMultiplier;
      coreMesh.rotation.y = -elapsedTime * 0.5 * speedMultiplier;

      particles.rotation.y = elapsedTime * 0.08 * speedMultiplier;
      particles.rotation.x = elapsedTime * 0.05 * speedMultiplier;

      // Pulsing material color & opacity based on mode
      if (currentMode === 'scanning') {
        icoMaterial.color.setHex(0xf39c12);
        icoMaterial.opacity = 0.6 + Math.sin(elapsedTime * 8) * 0.25;
      } else if (currentMode === 'downloading') {
        icoMaterial.color.setHex(0x2ecc71);
        icoMaterial.opacity = 0.55 + Math.sin(elapsedTime * 4) * 0.2;
      } else {
        icoMaterial.color.setHex(0xe05c3b);
        icoMaterial.opacity = 0.45;
      }

      renderer.render(scene, camera);
    };

    animate();

    // 7. Responsive Resize Listener
    const handleResize = () => {
      if (!container) return;
      const newW = container.clientWidth || 300;
      const newH = container.clientHeight || 300;
      camera.aspect = newW / newH;
      camera.updateProjectionMatrix();
      renderer.setSize(newW, newH);
    };

    window.addEventListener('resize', handleResize);

    // Cleanup
    return () => {
      cancelAnimationFrame(animationFrameId);
      if (interactive) {
        window.removeEventListener('mousemove', handleMouseMove);
      }
      window.removeEventListener('resize', handleResize);

      icoGeometry.dispose();
      icoMaterial.dispose();
      coreGeometry.dispose();
      coreMaterial.dispose();
      particleGeometry.dispose();
      particleMaterial.dispose();
      renderer.dispose();

      if (renderer.domElement && container.contains(renderer.domElement)) {
        container.removeChild(renderer.domElement);
      }
    };
  }, [interactive]);

  return (
    <div
      ref={containerRef}
      style={{
        width: '100%',
        height: height,
        position: 'relative',
        overflow: 'hidden',
        pointerEvents: interactive ? 'auto' : 'none',
      }}
      data-testid="three-canvas-container"
    />
  );
}
