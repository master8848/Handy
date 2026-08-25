import React from "react";
import { MainSidebar, type MainNavId } from "./MainSidebar";

export const ResponsiveSidebarHost: React.FC<{
  active: MainNavId;
  onChange: (id: MainNavId) => void;
  collapsed: boolean;
  onToggle: () => void;
  children: React.ReactNode;
}> = ({ active, onChange, collapsed, onToggle, children }) => {
  return (
    <div className="flex-1 flex min-h-0 overflow-hidden">
      <MainSidebar active={active} onChange={onChange} collapsed={collapsed} onToggleCollapsed={onToggle} />
      <div className="flex-1 overflow-y-auto">{children}</div>
    </div>
  );
};
