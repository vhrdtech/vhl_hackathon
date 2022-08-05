The goal is of this hackathon is to discover what is needed to generate xPI code from vhL.
In order to do it on a realistic task, LED lighting controller was selected as a small project that will also be beneficial in the Lab.

* LED controller board should be connect to the ECBridge through CAN Bus, which in turn will be connected via Ethernet.
* xPI over UAVCAN over CAN Bus
* xPI over WebSockets over Ethernet

Ideally several pieces of SW and HW should be created:
1) vhL source of the LED lighting controller
2) Supporting code for the embedded platform
3) Supporting code for the Rust side
4) Supporting code for the Dart side
5) Hand written and then generated code for the embedded platform
6) Hand written and then generated client code for Rust side
7) Hand written and then generated client code for the Dart/Flutter side

8) Setup a Flutter+Rust+bridge project, create simple UI
9) Bring up ECBridge + Ethernet + WebSockets
10) Implement uavcan-llr enough to use here
