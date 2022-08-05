The goal is of this hackathon is to discover what is needed to generate xPI code from vhL.
In order to do it on a realistic task, LED lighting controller was selected as a small project that will also be beneficial in the Lab.

![](https://user-images.githubusercontent.com/6066470/183160433-12f844c8-dbfb-4e7a-9720-eb0090e5c615.png)

* LED controller board should be connect to the ECBridge through CAN Bus, which in turn will be connected via Ethernet.
* xPI over UAVCAN over CAN Bus
* xPI over WebSockets over Ethernet

Ideally several pieces of SW and HW should be created:
- [x] vhL source of the LED lighting controller - [done](https://github.com/vhrdtech/vhl_hw/blob/main/led_ctrl/led_ctrl.vhl)
- [ ] Supporting code for the embedded platform (bbqueue integration, xPI Node impl for UAVCAN)
- [ ] Supporting code for the Rust side (WebSockets client, xPI Node impl for async)
- [ ] Supporting code for the Dart side (async bridge with Rust, bridge <-> UI model classes)
- [ ] Common supporting code (xPI Event, SerDes connection)
- [ ] Hand written and then generated code for the embedded platform
- [ ] Hand written and then generated client code for Rust side
- [ ] Hand written and then generated client code for the Dart/Flutter side

- [ ] Setup a Flutter+Rust+bridge project, create simple UI
- [ ] Bring up ECBridge + Ethernet + WebSockets
- [ ] Implement [uavcan-llr](https://github.com/vhrdtech/uavcan-llr) enough to use here
