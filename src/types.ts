export interface Trigger {
  id: string;
  start_time: string;
  end_time: string;
  osc_address: string;
  osc_args: string[];
}

export interface AudioDevice {
  name: string;
  channels: number;
}
