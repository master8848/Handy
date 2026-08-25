import React, { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { commands, type HarperStatus } from "@/bindings";
import { useSettings } from "../../hooks/useSettings";

const HarperStatusIndicator: React.FC = () => {
  const { t } = useTranslation();
  const { settings } = useSettings();
  const [status, setStatus] = useState<HarperStatus | null>(null);

  useEffect(() => {
    const pollStatus = async () => {
      try {
        const result = await commands.harperStatus();
        if (result.status === "ok") {
          setStatus(result.data);
        }
      } catch (error) {
        setStatus(null);
      }
    };

    pollStatus();
    const interval = setInterval(pollStatus, 4000);
    return () => clearInterval(interval);
  }, [settings?.spell_check_enabled]);

  const isReady = status?.enabled && status?.initialized;
  const isInitializing = status?.enabled && !status?.initialized;

  let dotClass = "bg-mid-gray";
  let tooltip = t("footer.harper.disabled");

  if (isReady) {
    dotClass = "bg-green-500";
    tooltip = t("footer.harper.ready");
  } else if (isInitializing) {
    dotClass = "bg-yellow-400 animate-pulse";
    tooltip = t("footer.harper.initializing");
  }

  return (
    <div className="flex items-center gap-2" title={tooltip}>
      <div className={`w-2 h-2 rounded-full ${dotClass}`} />
      <span className="text-xs text-text/60">{t("footer.harper.label")}</span>
    </div>
  );
};

export default HarperStatusIndicator;
