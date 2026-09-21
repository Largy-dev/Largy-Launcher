/** A small pixel-art grass block, used as the default icon for vanilla instances (no modpack icon to show). */
export function MinecraftGrassIcon({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 16 16" shapeRendering="crispEdges" className={className} aria-hidden="true">
      <rect width="16" height="16" fill="#8a5a35" />
      <rect width="16" height="6" fill="#5a9e37" />
      <rect y="4" width="16" height="2" fill="#4c8a2f" />
      <rect x="1" y="9" width="2" height="2" fill="#6b3f22" />
      <rect x="7" y="11" width="2" height="2" fill="#734526" />
      <rect x="12" y="8" width="2" height="2" fill="#6b3f22" />
      <rect x="4" y="13" width="2" height="2" fill="#734526" />
      <rect x="10" y="13" width="2" height="2" fill="#6b3f22" />
    </svg>
  );
}
